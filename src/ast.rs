use quote::ToTokens;
use std::path::Path;
use syn::spanned::Spanned;
use syn::{ImplItem, Item, TraitItem};
use tree_sitter::{Language, Node, Parser};

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

pub fn parse_items_for_path(path: &Path, source: &str) -> Vec<CodeItem> {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        parse_items(ext, source)
    } else {
        Vec::new()
    }
}

pub fn parse_items(extension: &str, source: &str) -> Vec<CodeItem> {
    let ext = extension.to_ascii_lowercase();
    match ext.as_str() {
        "rs" => parse_rust_items(source),
        "ts" | "mts" | "cts" => parse_typescript_items(source, false),
        "tsx" => parse_typescript_items(source, true),
        "js" | "jsx" | "mjs" | "cjs" => parse_javascript_items(source),
        "py" => parse_python_items(source),
        "cs" => parse_csharp_items(source),
        "go" => parse_go_items(source),
        "java" => parse_java_items(source),
        "c" | "h" => parse_c_items(source),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => parse_cpp_items(source),
        _ => Vec::new(),
    }
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

fn parse_tree_sitter_ast<F>(language: Language, source: &str, collector: F) -> Vec<CodeItem>
where
    F: FnOnce(Node, &str, &mut Vec<CodeItem>),
{
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let root = tree.root_node();
    let mut items = Vec::new();
    collector(root, source, &mut items);
    items
}

fn extract_class_tokens<F>(node: Node, source: &str, is_method: F) -> String
where
    F: Fn(&str) -> bool,
{
    let mut method_ranges = Vec::new();
    collect_method_ranges(node, &is_method, &mut method_ranges);
    method_ranges.sort_by_key(|r| r.0);

    let start = node.start_byte();
    let end = node.end_byte();
    let mut result = String::new();
    let mut current = start;

    for (m_start, m_end) in method_ranges {
        if m_start > current && m_start <= end {
            result.push_str(&source[current..m_start]);
        }
        current = current.max(m_end);
    }
    if current < end {
        result.push_str(&source[current..end]);
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_method_ranges<F>(node: Node, is_method: &F, out: &mut Vec<(usize, usize)>)
where
    F: Fn(&str) -> bool,
{
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_method(child.kind()) {
            out.push((child.start_byte(), child.end_byte()));
        } else if !matches!(
            child.kind(),
            "class_declaration"
                | "abstract_class_declaration"
                | "class_definition"
                | "struct_declaration"
                | "class_specifier"
                | "struct_specifier"
                | "union_specifier"
                | "enum_specifier"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "namespace_definition"
                | "type_declaration"
        ) {
            collect_method_ranges(child, is_method, out);
        }
    }
}

fn is_ts_js_method(kind: &str) -> bool {
    matches!(
        kind,
        "method_definition" | "function_declaration" | "generator_function_declaration"
    )
}

fn is_python_method(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition" | "async_function_definition" | "decorated_definition"
    )
}

fn is_csharp_method(kind: &str) -> bool {
    matches!(
        kind,
        "method_declaration"
            | "constructor_declaration"
            | "destructor_declaration"
            | "local_function_statement"
    )
}

fn is_java_method(kind: &str) -> bool {
    matches!(
        kind,
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration"
    )
}

fn is_c_method(kind: &str) -> bool {
    matches!(kind, "function_definition")
}

fn is_cpp_method(kind: &str) -> bool {
    matches!(kind, "function_definition")
}

pub fn parse_typescript_items(source: &str, is_tsx: bool) -> Vec<CodeItem> {
    let language: Language = if is_tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_ts_js_node(root, src, None, out);
    })
}

pub fn parse_javascript_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_javascript::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_ts_js_node(root, src, None, out);
    })
}

fn visit_ts_js_node(node: Node, source: &str, parent_class: Option<&str>, out: &mut Vec<CodeItem>) {
    match node.kind() {
        "class_declaration" | "abstract_class_declaration" | "class" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: extract_class_tokens(node, source, is_ts_js_method),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_ts_js_node(child, source, Some(name), out);
                    }
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.id() != name_node.id() {
                            visit_ts_js_node(child, source, Some(name), out);
                        }
                    }
                }
                return;
            }
        }
        "interface_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_ts_js_node(child, source, Some(name), out);
                    }
                }
                return;
            }
        }
        "enum_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    parent: parent_class.map(String::from),
                });
                return;
            }
        }
        "type_alias_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    parent: parent_class.map(String::from),
                });
                return;
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_ts_js_node(child, source, parent_class, out);
                    }
                }
                return;
            }
        }
        "method_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_ts_js_node(child, source, parent_class, out);
                    }
                }
                return;
            }
        }
        "variable_declarator" => {
            if let (Some(name_node), Some(value_node)) = (
                node.child_by_field_name("name"),
                node.child_by_field_name("value"),
            ) {
                match value_node.kind() {
                    "arrow_function" | "function_expression" | "generator_function" => {
                        let name = &source[name_node.byte_range()];
                        out.push(CodeItem {
                            name: name.to_string(),
                            kind: CodeItemKind::Function,
                            line: Some(node.start_position().row + 1),
                            tokens: source[node.byte_range()].to_string(),
                            parent: parent_class.map(String::from),
                        });
                        if let Some(body) = value_node.child_by_field_name("body") {
                            let mut cursor = body.walk();
                            for child in body.children(&mut cursor) {
                                visit_ts_js_node(child, source, parent_class, out);
                            }
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_ts_js_node(child, source, parent_class, out);
    }
}

pub fn parse_python_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_python::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_python_node(root, src, None, out);
    })
}

fn visit_python_node(
    node: Node,
    source: &str,
    parent_class: Option<&str>,
    out: &mut Vec<CodeItem>,
) {
    match node.kind() {
        "class_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: extract_class_tokens(node, source, is_python_method),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_python_node(child, source, Some(name), out);
                    }
                }
                return;
            }
        }
        "function_definition" | "async_function_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_python_node(child, source, parent_class, out);
                    }
                }
                return;
            }
        }
        "decorated_definition" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "function_definition"
                    || child.kind() == "async_function_definition"
                {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = &source[name_node.byte_range()];
                        out.push(CodeItem {
                            name: name.to_string(),
                            kind: CodeItemKind::Function,
                            line: Some(node.start_position().row + 1),
                            tokens: source[node.byte_range()].to_string(),
                            parent: parent_class.map(String::from),
                        });
                        if let Some(body) = child.child_by_field_name("body") {
                            let mut b_cursor = body.walk();
                            for b_child in body.children(&mut b_cursor) {
                                visit_python_node(b_child, source, parent_class, out);
                            }
                        }
                        return;
                    }
                } else if child.kind() == "class_definition" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = &source[name_node.byte_range()];
                        out.push(CodeItem {
                            name: name.to_string(),
                            kind: CodeItemKind::Class,
                            line: Some(node.start_position().row + 1),
                            tokens: extract_class_tokens(node, source, is_python_method),
                            parent: parent_class.map(String::from),
                        });
                        if let Some(body) = child.child_by_field_name("body") {
                            let mut b_cursor = body.walk();
                            for b_child in body.children(&mut b_cursor) {
                                visit_python_node(b_child, source, Some(name), out);
                            }
                        }
                        return;
                    }
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_python_node(child, source, parent_class, out);
    }
}

pub fn parse_csharp_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_c_sharp::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_csharp_node(root, src, None, out);
    })
}

fn visit_csharp_node(
    node: Node,
    source: &str,
    parent_class: Option<&str>,
    out: &mut Vec<CodeItem>,
) {
    match node.kind() {
        "class_declaration"
        | "struct_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "record_struct_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: extract_class_tokens(node, source, is_csharp_method),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_csharp_node(child, source, Some(name), out);
                    }
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.id() != name_node.id() {
                            visit_csharp_node(child, source, Some(name), out);
                        }
                    }
                }
                return;
            }
        }
        "method_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_csharp_node(child, source, parent_class, out);
                    }
                }
                return;
            }
        }
        "constructor_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_csharp_node(child, source, parent_class, out);
                    }
                }
                return;
            }
        }
        "local_function_statement" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_csharp_node(child, source, parent_class, out);
    }
}

pub fn parse_go_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_go::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_go_node(root, src, None, out);
    })
}

fn extract_go_receiver_type<'a>(receiver_node: Node, source: &'a str) -> Option<&'a str> {
    fn find_type_ident<'a>(node: Node, source: &'a str) -> Option<&'a str> {
        if node.kind() == "type_identifier" {
            return Some(&source[node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(res) = find_type_ident(child, source) {
                return Some(res);
            }
        }
        None
    }
    find_type_ident(receiver_node, source)
}

fn visit_go_node(node: Node, source: &str, parent_class: Option<&str>, out: &mut Vec<CodeItem>) {
    match node.kind() {
        "type_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "type_spec" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = &source[name_node.byte_range()];
                        out.push(CodeItem {
                            name: name.to_string(),
                            kind: CodeItemKind::Class,
                            line: Some(child.start_position().row + 1),
                            tokens: source[child.byte_range()]
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" "),
                            parent: parent_class.map(String::from),
                        });
                    }
                }
            }
            return;
        }
        "type_spec" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    parent: parent_class.map(String::from),
                });
            }
            return;
        }
        "function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
            }
            return;
        }
        "method_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                let parent = node
                    .child_by_field_name("receiver")
                    .and_then(|recv| extract_go_receiver_type(recv, source))
                    .map(String::from)
                    .or_else(|| parent_class.map(String::from));
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent,
                });
            }
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_go_node(child, source, parent_class, out);
    }
}

pub fn parse_java_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_java::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_java_node(root, src, None, out);
    })
}

fn visit_java_node(node: Node, source: &str, parent_class: Option<&str>, out: &mut Vec<CodeItem>) {
    match node.kind() {
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: extract_class_tokens(node, source, is_java_method),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_java_node(child, source, Some(name), out);
                    }
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.id() != name_node.id() {
                            visit_java_node(child, source, Some(name), out);
                        }
                    }
                }
                return;
            }
        }
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Function,
                    line: Some(node.start_position().row + 1),
                    tokens: source[node.byte_range()].to_string(),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_java_node(child, source, parent_class, out);
                    }
                }
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_java_node(child, source, parent_class, out);
    }
}

pub fn parse_c_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_c::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_c_node(root, src, None, out);
    })
}

fn extract_c_declarator_name<'a>(node: Node, source: &'a str) -> Option<&'a str> {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" => Some(&source[node.byte_range()]),
        _ => {
            if let Some(decl) = node.child_by_field_name("declarator") {
                extract_c_declarator_name(decl, source)
            } else {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if let Some(name) = extract_c_declarator_name(child, source) {
                        return Some(name);
                    }
                }
                None
            }
        }
    }
}

fn visit_c_node(node: Node, source: &str, parent_class: Option<&str>, out: &mut Vec<CodeItem>) {
    match node.kind() {
        "struct_specifier" | "union_specifier" | "enum_specifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                if !name.is_empty() {
                    out.push(CodeItem {
                        name: name.to_string(),
                        kind: CodeItemKind::Class,
                        line: Some(node.start_position().row + 1),
                        tokens: extract_class_tokens(node, source, is_c_method),
                        parent: parent_class.map(String::from),
                    });
                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            visit_c_node(child, source, Some(name), out);
                        }
                    }
                    return;
                }
            }
        }
        "type_definition" => {
            if let Some(decl_node) = node.child_by_field_name("declarator") {
                if let Some(name) = extract_c_declarator_name(decl_node, source) {
                    out.push(CodeItem {
                        name: name.to_string(),
                        kind: CodeItemKind::Class,
                        line: Some(node.start_position().row + 1),
                        tokens: source[node.byte_range()]
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" "),
                        parent: parent_class.map(String::from),
                    });
                    return;
                }
            }
        }
        "function_definition" => {
            if let Some(decl_node) = node.child_by_field_name("declarator") {
                if let Some(name) = extract_c_declarator_name(decl_node, source) {
                    out.push(CodeItem {
                        name: name.to_string(),
                        kind: CodeItemKind::Function,
                        line: Some(node.start_position().row + 1),
                        tokens: source[node.byte_range()].to_string(),
                        parent: parent_class.map(String::from),
                    });
                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            visit_c_node(child, source, parent_class, out);
                        }
                    }
                    return;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_c_node(child, source, parent_class, out);
    }
}

pub fn parse_cpp_items(source: &str) -> Vec<CodeItem> {
    let language: Language = tree_sitter_cpp::LANGUAGE.into();
    parse_tree_sitter_ast(language, source, |root, src, out| {
        visit_cpp_node(root, src, None, out);
    })
}

fn extract_cpp_declarator_info<'a>(
    node: Node,
    source: &'a str,
) -> (Option<&'a str>, Option<&'a str>) {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" | "destructor_name"
        | "operator_name" => (Some(&source[node.byte_range()]), None),
        "qualified_identifier" => {
            let mut current = node;
            let mut last_scope = None;
            while current.kind() == "qualified_identifier" {
                if let Some(scope_node) = current.child_by_field_name("scope") {
                    let scope_text = &source[scope_node.byte_range()];
                    last_scope = scope_text.split("::").last();
                }
                if let Some(name_node) = current.child_by_field_name("name") {
                    if name_node.kind() == "qualified_identifier" {
                        current = name_node;
                    } else {
                        return (Some(&source[name_node.byte_range()]), last_scope);
                    }
                } else {
                    break;
                }
            }
            (Some(&source[current.byte_range()]), last_scope)
        }
        _ => {
            if let Some(decl) = node.child_by_field_name("declarator") {
                extract_cpp_declarator_info(decl, source)
            } else {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    let (name, scope) = extract_cpp_declarator_info(child, source);
                    if name.is_some() {
                        return (name, scope);
                    }
                }
                (None, None)
            }
        }
    }
}

fn visit_cpp_node(node: Node, source: &str, parent_class: Option<&str>, out: &mut Vec<CodeItem>) {
    match node.kind() {
        "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source[name_node.byte_range()];
                if !name.is_empty() {
                    out.push(CodeItem {
                        name: name.to_string(),
                        kind: CodeItemKind::Class,
                        line: Some(node.start_position().row + 1),
                        tokens: extract_class_tokens(node, source, is_cpp_method),
                        parent: parent_class.map(String::from),
                    });
                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            visit_cpp_node(child, source, Some(name), out);
                        }
                    } else {
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            if child.id() != name_node.id() {
                                visit_cpp_node(child, source, Some(name), out);
                            }
                        }
                    }
                    return;
                }
            }
        }
        "namespace_definition" => {
            let name_opt = node
                .child_by_field_name("name")
                .map(|n| &source[n.byte_range()]);
            if let Some(name) = name_opt {
                out.push(CodeItem {
                    name: name.to_string(),
                    kind: CodeItemKind::Class,
                    line: Some(node.start_position().row + 1),
                    tokens: extract_class_tokens(node, source, is_cpp_method),
                    parent: parent_class.map(String::from),
                });
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        visit_cpp_node(child, source, Some(name), out);
                    }
                } else {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        visit_cpp_node(child, source, Some(name), out);
                    }
                }
                return;
            }
        }
        "type_definition" => {
            if let Some(decl_node) = node.child_by_field_name("declarator") {
                let (name_opt, _) = extract_cpp_declarator_info(decl_node, source);
                if let Some(name) = name_opt {
                    out.push(CodeItem {
                        name: name.to_string(),
                        kind: CodeItemKind::Class,
                        line: Some(node.start_position().row + 1),
                        tokens: source[node.byte_range()]
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" "),
                        parent: parent_class.map(String::from),
                    });
                    return;
                }
            }
        }
        "function_definition" => {
            if let Some(decl_node) = node.child_by_field_name("declarator") {
                let (name_opt, scope_opt) = extract_cpp_declarator_info(decl_node, source);
                if let Some(name) = name_opt {
                    let parent = parent_class
                        .map(String::from)
                        .or_else(|| scope_opt.map(String::from));
                    out.push(CodeItem {
                        name: name.to_string(),
                        kind: CodeItemKind::Function,
                        line: Some(node.start_position().row + 1),
                        tokens: source[node.byte_range()].to_string(),
                        parent,
                    });
                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            visit_cpp_node(child, source, parent_class, out);
                        }
                    }
                    return;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_cpp_node(child, source, parent_class, out);
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

    #[test]
    fn test_parse_typescript_items() {
        let ts_code = r#"
export interface IUser {
    id: string;
}

export class UserService {
    getUser(id: string): IUser {
        return { id };
    }
}

export const fetchUser = async (id: string) => {
    return id;
};

export function deleteUser(id: string): void {}
"#;
        let items = parse_typescript_items(ts_code, false);
        assert_eq!(items.len(), 5);

        assert_eq!(items[0].name, "IUser");
        assert_eq!(items[0].kind, CodeItemKind::Class);
        assert_eq!(items[0].line, Some(2));

        assert_eq!(items[1].name, "UserService");
        assert_eq!(items[1].kind, CodeItemKind::Class);
        assert_eq!(items[1].line, Some(6));

        assert_eq!(items[2].name, "getUser");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert_eq!(items[2].line, Some(7));
        assert_eq!(items[2].parent.as_deref(), Some("UserService"));

        assert_eq!(items[3].name, "fetchUser");
        assert_eq!(items[3].kind, CodeItemKind::Function);
        assert_eq!(items[3].line, Some(12));

        assert_eq!(items[4].name, "deleteUser");
        assert_eq!(items[4].kind, CodeItemKind::Function);
        assert_eq!(items[4].line, Some(16));
    }

    #[test]
    fn test_parse_javascript_items() {
        let js_code = r#"
class PaymentProcessor {
    processPayment(amount) {
        return true;
    }
}

const refund = function(id) {
    return false;
};
"#;
        let items = parse_javascript_items(js_code);
        assert_eq!(items.len(), 3);

        assert_eq!(items[0].name, "PaymentProcessor");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "processPayment");
        assert_eq!(items[1].kind, CodeItemKind::Function);
        assert_eq!(items[1].parent.as_deref(), Some("PaymentProcessor"));

        assert_eq!(items[2].name, "refund");
        assert_eq!(items[2].kind, CodeItemKind::Function);
    }

    #[test]
    fn test_parse_python_items() {
        let py_code = r#"
class DataStore:
    def __init__(self, name: str):
        self.name = name

    def save(self, data):
        pass

def global_compute(x: int) -> int:
    return x * 2
"#;
        let items = parse_python_items(py_code);
        assert_eq!(items.len(), 4);

        assert_eq!(items[0].name, "DataStore");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "__init__");
        assert_eq!(items[1].kind, CodeItemKind::Function);
        assert_eq!(items[1].parent.as_deref(), Some("DataStore"));

        assert_eq!(items[2].name, "save");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert_eq!(items[2].parent.as_deref(), Some("DataStore"));

        assert_eq!(items[3].name, "global_compute");
        assert_eq!(items[3].kind, CodeItemKind::Function);
        assert!(items[3].parent.is_none());
    }

    #[test]
    fn test_parse_csharp_items() {
        let cs_code = r#"
namespace MyApp.Services
{
    public class OrderManager
    {
        public OrderManager() {}

        public bool CreateOrder(int id)
        {
            return true;
        }
    }

    public interface IOrderService {}
}
"#;
        let items = parse_csharp_items(cs_code);
        assert_eq!(items.len(), 4);

        assert_eq!(items[0].name, "OrderManager");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "OrderManager");
        assert_eq!(items[1].kind, CodeItemKind::Function);
        assert_eq!(items[1].parent.as_deref(), Some("OrderManager"));

        assert_eq!(items[2].name, "CreateOrder");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert_eq!(items[2].parent.as_deref(), Some("OrderManager"));

        assert_eq!(items[3].name, "IOrderService");
        assert_eq!(items[3].kind, CodeItemKind::Class);
    }

    #[test]
    fn test_parse_go_items() {
        let go_code = r#"
package service

type Config struct {
    Port int
}

type Processor interface {
    Process() error
}

func InitConfig() *Config {
    return &Config{Port: 8080}
}

func (c *Config) GetPort() int {
    return c.Port
}
"#;
        let items = parse_go_items(go_code);
        assert_eq!(items.len(), 4);

        assert_eq!(items[0].name, "Config");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "Processor");
        assert_eq!(items[1].kind, CodeItemKind::Class);

        assert_eq!(items[2].name, "InitConfig");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert!(items[2].parent.is_none());

        assert_eq!(items[3].name, "GetPort");
        assert_eq!(items[3].kind, CodeItemKind::Function);
        assert_eq!(items[3].parent.as_deref(), Some("Config"));
    }

    #[test]
    fn test_parse_java_items() {
        let java_code = r#"
package com.example;

public class OrderService {
    public OrderService() {}

    public boolean placeOrder(int id) {
        return true;
    }
}

public interface IHandler {
    void handle();
}

public record Point(int x, int y) {}
"#;
        let items = parse_java_items(java_code);
        assert_eq!(items.len(), 6);

        assert_eq!(items[0].name, "OrderService");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "OrderService");
        assert_eq!(items[1].kind, CodeItemKind::Function);
        assert_eq!(items[1].parent.as_deref(), Some("OrderService"));

        assert_eq!(items[2].name, "placeOrder");
        assert_eq!(items[2].kind, CodeItemKind::Function);
        assert_eq!(items[2].parent.as_deref(), Some("OrderService"));

        assert_eq!(items[3].name, "IHandler");
        assert_eq!(items[3].kind, CodeItemKind::Class);

        assert_eq!(items[4].name, "handle");
        assert_eq!(items[4].kind, CodeItemKind::Function);
        assert_eq!(items[4].parent.as_deref(), Some("IHandler"));

        assert_eq!(items[5].name, "Point");
        assert_eq!(items[5].kind, CodeItemKind::Class);
    }

    #[test]
    fn test_parse_c_items() {
        let c_code = r#"
struct Vector {
    float x;
    float y;
};

enum Status {
    OK = 0,
    ERROR = 1
};

int calculate_sum(int a, int b) {
    return a + b;
}
"#;
        let items = parse_c_items(c_code);
        assert_eq!(items.len(), 3);

        assert_eq!(items[0].name, "Vector");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "Status");
        assert_eq!(items[1].kind, CodeItemKind::Class);

        assert_eq!(items[2].name, "calculate_sum");
        assert_eq!(items[2].kind, CodeItemKind::Function);
    }

    #[test]
    fn test_parse_cpp_items() {
        let cpp_code = r#"
namespace Math {
    struct Vector3 {
        float x, y, z;
    };

    class Calculator {
    public:
        int add(int a, int b) {
            return a + b;
        }
    };
}

int Math::Calculator::subtract(int a, int b) {
    return a - b;
}
"#;
        let items = parse_cpp_items(cpp_code);
        assert_eq!(items.len(), 5);

        assert_eq!(items[0].name, "Math");
        assert_eq!(items[0].kind, CodeItemKind::Class);

        assert_eq!(items[1].name, "Vector3");
        assert_eq!(items[1].kind, CodeItemKind::Class);

        assert_eq!(items[2].name, "Calculator");
        assert_eq!(items[2].kind, CodeItemKind::Class);

        assert_eq!(items[3].name, "add");
        assert_eq!(items[3].kind, CodeItemKind::Function);
        assert_eq!(items[3].parent.as_deref(), Some("Calculator"));

        assert_eq!(items[4].name, "subtract");
        assert_eq!(items[4].kind, CodeItemKind::Function);
        assert_eq!(items[4].parent.as_deref(), Some("Calculator"));
    }
}
