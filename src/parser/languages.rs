use tree_sitter::Language;

/// Returns the tree-sitter Language for the given language name.
pub fn get_language(name: &str) -> Option<Language> {
    match name {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        "ruby" => Some(tree_sitter_ruby::LANGUAGE.into()),
        _ => None,
    }
}

/// Returns the tree-sitter query patterns for symbol extraction in the given language.
/// Adapted from Zed editor's outline.scm patterns.
pub fn get_symbol_query(language: &str) -> Option<&'static str> {
    match language {
        "rust" => Some(RUST_QUERY),
        "python" => Some(PYTHON_QUERY),
        "typescript" => Some(TYPESCRIPT_QUERY),
        "javascript" => Some(JAVASCRIPT_QUERY),
        "go" => Some(GO_QUERY),
        "java" => Some(JAVA_QUERY),
        "c" => Some(C_QUERY),
        "cpp" => Some(CPP_QUERY),
        "ruby" => Some(RUBY_QUERY),
        _ => None,
    }
}

// ── Rust ──────────────────────────────────────────────────────────────────

const RUST_QUERY: &str = r#"
(function_item
  name: (identifier) @fn.name
  parameters: (parameters) @fn.params
  return_type: (_)? @fn.return_type
  body: (block) @fn.body) @fn.def

(struct_item
  name: (type_identifier) @struct.name
  body: (_)? @struct.body) @struct.def

(enum_item
  name: (type_identifier) @enum.name
  body: (enum_variant_list)? @enum.body) @enum.def

(trait_item
  name: (type_identifier) @trait.name
  body: (declaration_list)? @trait.body) @trait.def

(impl_item
  trait: (_)? @impl.trait
  type: (_) @impl.type
  body: (declaration_list) @impl.body) @impl.def

(use_declaration
  argument: (_) @import.path) @import.def

(mod_item
  name: (identifier) @mod.name) @mod.def

(type_item
  name: (type_identifier) @type_alias.name) @type_alias.def

(const_item
  name: (identifier) @const.name) @const.def

(static_item
  name: (identifier) @static.name) @static.def
"#;

// ── Python ────────────────────────────────────────────────────────────────

const PYTHON_QUERY: &str = r#"
(function_definition
  name: (identifier) @fn.name
  parameters: (parameters) @fn.params
  return_type: (_)? @fn.return_type
  body: (block) @fn.body) @fn.def

(class_definition
  name: (identifier) @class.name
  superclasses: (argument_list)? @class.bases
  body: (block) @class.body) @class.def

(import_statement) @import.def

(import_from_statement
  module_name: (_)? @import.module) @import.def

(decorated_definition) @decorated.def
"#;

// ── TypeScript/TSX ────────────────────────────────────────────────────────

const TYPESCRIPT_QUERY: &str = r#"
(function_declaration
  name: (identifier) @fn.name
  parameters: (formal_parameters) @fn.params
  return_type: (_)? @fn.return_type
  body: (statement_block) @fn.body) @fn.def

(class_declaration
  name: (type_identifier) @class.name
  body: (class_body) @class.body) @class.def

(interface_declaration
  name: (type_identifier) @interface.name
  body: (interface_body)? @interface.body) @interface.def

(type_alias_declaration
  name: (type_identifier) @type_alias.name
  value: (_) @type_alias.value) @type_alias.def

(enum_declaration
  name: (identifier) @enum.name
  body: (enum_body) @enum.body) @enum.def

(method_definition
  name: (property_identifier) @method.name
  parameters: (formal_parameters) @method.params
  return_type: (_)? @method.return_type
  body: (statement_block) @method.body) @method.def

(import_statement) @import.def

(export_statement) @export.def

(lexical_declaration
  (variable_declarator
    name: (identifier) @var.name
    value: (arrow_function)? @var.arrow)) @var.def
"#;

// ── JavaScript ────────────────────────────────────────────────────────────

const JAVASCRIPT_QUERY: &str = r#"
(function_declaration
  name: (identifier) @fn.name
  parameters: (formal_parameters) @fn.params
  body: (statement_block) @fn.body) @fn.def

(class_declaration
  name: (identifier) @class.name
  body: (class_body) @class.body) @class.def

(method_definition
  name: (property_identifier) @method.name
  parameters: (formal_parameters) @method.params
  body: (statement_block) @method.body) @method.def

(import_statement) @import.def

(export_statement) @export.def

(lexical_declaration
  (variable_declarator
    name: (identifier) @var.name
    value: (arrow_function)? @var.arrow)) @var.def
"#;

// ── Go ────────────────────────────────────────────────────────────────────

const GO_QUERY: &str = r#"
(function_declaration
  name: (identifier) @fn.name
  parameters: (parameter_list) @fn.params
  result: (_)? @fn.return_type
  body: (block) @fn.body) @fn.def

(method_declaration
  receiver: (parameter_list) @method.receiver
  name: (field_identifier) @method.name
  parameters: (parameter_list) @method.params
  result: (_)? @method.return_type
  body: (block) @method.body) @method.def

(type_declaration
  (type_spec
    name: (type_identifier) @type.name
    type: (struct_type)? @type.struct
    type: (interface_type)? @type.interface)) @type.def

(import_declaration) @import.def

(package_clause
  (package_identifier) @package.name) @package.def
"#;

// ── Java ──────────────────────────────────────────────────────────────────

const JAVA_QUERY: &str = r#"
(class_declaration
  name: (identifier) @class.name
  body: (class_body) @class.body) @class.def

(interface_declaration
  name: (identifier) @interface.name
  body: (interface_body)? @interface.body) @interface.def

(enum_declaration
  name: (identifier) @enum.name
  body: (enum_body) @enum.body) @enum.def

(method_declaration
  name: (identifier) @method.name
  parameters: (formal_parameters) @method.params
  body: (block)? @method.body) @method.def

(constructor_declaration
  name: (identifier) @fn.name
  parameters: (formal_parameters) @fn.params
  body: (constructor_body) @fn.body) @fn.def

(import_declaration) @import.def
"#;

// ── C ─────────────────────────────────────────────────────────────────────

const C_QUERY: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @fn.name
    parameters: (parameter_list) @fn.params)
  body: (compound_statement) @fn.body) @fn.def

(struct_specifier
  name: (type_identifier) @struct.name
  body: (field_declaration_list)? @struct.body) @struct.def

(enum_specifier
  name: (type_identifier) @enum.name
  body: (enumerator_list)? @enum.body) @enum.def

(type_definition
  declarator: (type_identifier) @type_alias.name) @type_alias.def

(preproc_include) @import.def
"#;

// ── C++ ───────────────────────────────────────────────────────────────────

const CPP_QUERY: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @fn.name
    parameters: (parameter_list) @fn.params)
  body: (compound_statement) @fn.body) @fn.def

(class_specifier
  name: (type_identifier) @class.name
  body: (field_declaration_list)? @class.body) @class.def

(struct_specifier
  name: (type_identifier) @struct.name
  body: (field_declaration_list)? @struct.body) @struct.def

(enum_specifier
  name: (type_identifier) @enum.name
  body: (enumerator_list)? @enum.body) @enum.def

(preproc_include) @import.def
"#;

// ── Ruby ──────────────────────────────────────────────────────────────────

const RUBY_QUERY: &str = r#"
(class
  name: (constant) @class.name
  body: (_)? @class.body) @class.def

(module
  name: (constant) @mod.name
  body: (_)? @mod.body) @mod.def

(method
  name: (identifier) @method.name
  parameters: (method_parameters)? @method.params
  body: (_)? @method.body) @method.def

(singleton_method
  name: (identifier) @fn.name
  parameters: (method_parameters)? @fn.params
  body: (_)? @fn.body) @fn.def
"#;
