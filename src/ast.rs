use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{ImplItem, Item, TraitItem};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeItemKind {
    Class,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeItem {
    pub name: String,
    pub kind: CodeItemKind,
    pub line: Option<usize>,
    pub tokens: String,
    pub parent: Option<String>,
}

pub fn parse_rust_items(source: &str) -> Vec<CodeItem> {
    let Ok(syntax_file) = syn::parse_file(source) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    collect_items_from_slice(&syntax_file.items, &mut items);
    items
}

fn collect_items_from_slice(raw_items: &[Item], out: &mut Vec<CodeItem>) {
    for item in raw_items {
        match item {
            Item::Struct(s) => {
                out.push(CodeItem {
                    name: s.ident.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(s.span().start().line),
                    tokens: s.to_token_stream().to_string(),
                    parent: None,
                });
            }
            Item::Enum(e) => {
                out.push(CodeItem {
                    name: e.ident.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(e.span().start().line),
                    tokens: e.to_token_stream().to_string(),
                    parent: None,
                });
            }
            Item::Trait(t) => {
                out.push(CodeItem {
                    name: t.ident.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(t.span().start().line),
                    tokens: t.to_token_stream().to_string(),
                    parent: None,
                });
                for trait_item in &t.items {
                    if let TraitItem::Fn(tf) = trait_item {
                        out.push(CodeItem {
                            name: tf.sig.ident.to_string(),
                            kind: CodeItemKind::Function,
                            line: Some(tf.span().start().line),
                            tokens: tf.to_token_stream().to_string(),
                            parent: Some(t.ident.to_string()),
                        });
                    }
                }
            }
            Item::Union(u) => {
                out.push(CodeItem {
                    name: u.ident.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(u.span().start().line),
                    tokens: u.to_token_stream().to_string(),
                    parent: None,
                });
            }
            Item::Fn(f) => {
                out.push(CodeItem {
                    name: f.sig.ident.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(f.span().start().line),
                    tokens: f.to_token_stream().to_string(),
                    parent: None,
                });
            }
            Item::Impl(imp) => {
                let self_type_name = imp.self_ty.to_token_stream().to_string();
                for impl_item in &imp.items {
                    if let ImplItem::Fn(mf) = impl_item {
                        out.push(CodeItem {
                            name: mf.sig.ident.to_string(),
                            kind: CodeItemKind::Function,
                            line: Some(mf.span().start().line),
                            tokens: mf.to_token_stream().to_string(),
                            parent: Some(self_type_name.clone()),
                        });
                    }
                }
            }
            Item::Mod(m) => {
                if let Some((_, ref sub_items)) = m.content {
                    collect_items_from_slice(sub_items, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_struct_and_impl_methods() {
        let code = r#"
pub struct UserAuth {
    pub username: String,
}

impl UserAuth {
    pub fn login(&self) -> bool {
        true
    }

    pub fn logout(&self) {}
}
"#;
        let items = parse_rust_items(code);
        assert_eq!(items.len(), 3);

        assert_eq!(items[0].name, "UserAuth");
        assert_eq!(items[0].kind, CodeItemKind::Class);
        assert_eq!(items[0].line, Some(2));
        assert!(items[0].parent.is_none());

        assert_eq!(items[1].name, "login");
        assert_eq!(items[1].kind, CodeItemKind::Function);
        assert_eq!(items[1].line, Some(7));
        assert_eq!(items[1].parent.as_deref(), Some("UserAuth"));

        assert_eq!(items[2].name, "logout");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert_eq!(items[2].line, Some(11));
        assert_eq!(items[2].parent.as_deref(), Some("UserAuth"));
    }

    #[test]
    fn test_parse_enum_trait_and_functions() {
        let code = r#"
pub enum Status {
    Active,
    Inactive,
}

pub trait Authenticatable {
    fn verify(&self) -> bool;
}

pub fn global_helper() -> i32 {
    42
}
"#;
        let items = parse_rust_items(code);
        assert_eq!(items.len(), 4);

        assert_eq!(items[0].name, "Status");
        assert_eq!(items[0].kind, CodeItemKind::Class);
        assert_eq!(items[0].line, Some(2));

        assert_eq!(items[1].name, "Authenticatable");
        assert_eq!(items[1].kind, CodeItemKind::Class);
        assert_eq!(items[1].line, Some(7));

        assert_eq!(items[2].name, "verify");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert_eq!(items[2].line, Some(8));
        assert_eq!(items[2].parent.as_deref(), Some("Authenticatable"));

        assert_eq!(items[3].name, "global_helper");
        assert_eq!(items[3].kind, CodeItemKind::Function);
        assert_eq!(items[3].line, Some(11));
    }

    #[test]
    fn test_invalid_rust_returns_empty() {
        let items = parse_rust_items("This is not valid rust code {{{");
        assert!(items.is_empty());
    }
}
