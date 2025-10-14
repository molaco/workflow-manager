use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Attribute, Lit, Type, PathArguments, GenericArgument};

#[proc_macro_derive(WorkflowDefinition, attributes(workflow, field))]
pub fn derive_workflow_definition(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract workflow metadata from #[workflow(...)]
    let workflow_meta = extract_workflow_meta(&input.attrs);

    // Extract field schemas from struct fields
    let field_schemas: Vec<proc_macro2::TokenStream> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                fields.named.iter().filter_map(|f| {
                    let name = f.ident.as_ref().unwrap().to_string();

                    // Skip the workflow_metadata field
                    if name == "workflow_metadata" {
                        return None;
                    }

                    let field_type = infer_field_type(&f.ty);
                    let (label, description, field_type_override, required_for_phases) = extract_field_meta(&f.attrs);
                    let cli_arg = extract_cli_arg(&f.attrs, &name);
                    let required = !is_option_type(&f.ty);
                    let default = extract_default(&f.attrs);

                    // Use override if provided, otherwise infer
                    let final_field_type = field_type_override.unwrap_or(field_type);

                    Some(quote! {
                        workflow_manager_sdk::FieldSchema {
                            name: #name.to_string(),
                            field_type: #final_field_type,
                            label: #label.to_string(),
                            description: #description.to_string(),
                            cli_arg: #cli_arg.to_string(),
                            required: #required,
                            default: #default,
                            required_for_phases: #required_for_phases,
                        }
                    })
                }).collect()
            }
            _ => panic!("WorkflowDefinition only supports named fields")
        },
        _ => panic!("WorkflowDefinition only supports structs")
    };

    let struct_name = &input.ident;
    let workflow_id = &workflow_meta.id;
    let workflow_name = &workflow_meta.name;
    let workflow_desc = &workflow_meta.description;

    let expanded = quote! {
        impl workflow_manager_sdk::WorkflowDefinition for #struct_name {
            fn metadata() -> workflow_manager_sdk::WorkflowMetadata {
                workflow_manager_sdk::WorkflowMetadata {
                    id: #workflow_id.to_string(),
                    name: #workflow_name.to_string(),
                    description: #workflow_desc.to_string(),
                }
            }

            fn fields() -> Vec<workflow_manager_sdk::FieldSchema> {
                vec![#(#field_schemas),*]
            }

            fn print_metadata(&self) {
                let metadata = workflow_manager_sdk::WorkflowMetadata {
                    id: #workflow_id.to_string(),
                    name: #workflow_name.to_string(),
                    description: #workflow_desc.to_string(),
                };
                let full_metadata = workflow_manager_sdk::FullWorkflowMetadata {
                    metadata,
                    fields: Self::fields(),
                };
                let json = serde_json::to_string_pretty(&full_metadata).unwrap();
                println!("{}", json);
            }
        }
    };

    TokenStream::from(expanded)
}

struct WorkflowMeta {
    id: String,
    name: String,
    description: String,
}

fn extract_workflow_meta(attrs: &[Attribute]) -> WorkflowMeta {
    for attr in attrs {
        if attr.path().is_ident("workflow") {
            let mut id = String::new();
            let mut name = String::new();
            let mut description = String::new();

            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("id") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        id = s.value();
                    }
                } else if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        name = s.value();
                    }
                } else if meta.path.is_ident("description") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        description = s.value();
                    }
                }
                Ok(())
            });

            return WorkflowMeta { id, name, description };
        }
    }

    panic!("Missing #[workflow(...)] attribute");
}

fn extract_field_meta(attrs: &[Attribute]) -> (String, String, Option<proc_macro2::TokenStream>, Option<proc_macro2::TokenStream>) {
    let mut label = String::new();
    let mut description = String::new();
    let mut field_type = None;
    let mut min = None;
    let mut max = None;
    let mut pattern = None;
    let mut total_phases = None;
    let mut phase = None;
    let mut required_for_phases = None;

    for attr in attrs {
        if attr.path().is_ident("field") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("label") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        label = s.value();
                    }
                } else if meta.path.is_ident("description") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        description = s.value();
                    }
                } else if meta.path.is_ident("type") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        field_type = Some(s.value());
                    }
                } else if meta.path.is_ident("min") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        min = s.value().parse::<i64>().ok();
                    }
                } else if meta.path.is_ident("max") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        max = s.value().parse::<i64>().ok();
                    }
                } else if meta.path.is_ident("pattern") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        pattern = Some(s.value());
                    }
                } else if meta.path.is_ident("total_phases") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        total_phases = s.value().parse::<usize>().ok();
                    }
                } else if meta.path.is_ident("phase") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        phase = s.value().parse::<usize>().ok();
                    }
                } else if meta.path.is_ident("required_for_phases") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        required_for_phases = Some(s.value());
                    }
                }
                Ok(())
            });
        }
    }

    // Build field type from parsed values
    let field_type_token = field_type.map(|ft| {
        match ft.as_str() {
            "text" => quote! { workflow_manager_sdk::FieldType::Text },
            "number" => {
                let min_token = min.map(|m| quote! { Some(#m) }).unwrap_or(quote! { None });
                let max_token = max.map(|m| quote! { Some(#m) }).unwrap_or(quote! { None });
                quote! { workflow_manager_sdk::FieldType::Number { min: #min_token, max: #max_token } }
            }
            "file_path" => {
                let pattern_token = pattern.map(|p| quote! { Some(#p.to_string()) }).unwrap_or(quote! { None });
                quote! { workflow_manager_sdk::FieldType::FilePath { pattern: #pattern_token } }
            }
            "phase_selector" => {
                let total = total_phases.unwrap_or(5);
                quote! { workflow_manager_sdk::FieldType::PhaseSelector { total_phases: #total } }
            }
            "state_file" => {
                let pattern_str = pattern.unwrap_or_else(|| "*.yaml".to_string());
                let phase_token = phase.map(|p| quote! { Some(#p) }).unwrap_or(quote! { None });
                quote! { workflow_manager_sdk::FieldType::StateFile { pattern: #pattern_str.to_string(), phase: #phase_token } }
            }
            _ => quote! { workflow_manager_sdk::FieldType::Text },
        }
    });

    // Build required_for_phases token (e.g., "0,1,2" -> Some(vec![0, 1, 2]))
    let required_for_phases_token = required_for_phases.map(|phases_str| {
        let phases: Vec<usize> = phases_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        quote! { Some(vec![#(#phases),*]) }
    });

    (label, description, field_type_token, Some(required_for_phases_token.unwrap_or(quote! { None })))
}

fn extract_cli_arg(attrs: &[Attribute], field_name: &str) -> String {
    for attr in attrs {
        if attr.path().is_ident("arg") {
            let mut long_name = None;

            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("long") {
                    if let Ok(value) = meta.value() {
                        let lit: Lit = value.parse()?;
                        if let Lit::Str(s) = lit {
                            long_name = Some(s.value());
                        }
                    } else {
                        // #[arg(long)] without value uses field name
                        long_name = Some(field_name.replace("_", "-"));
                    }
                }
                Ok(())
            });

            if let Some(name) = long_name {
                return format!("--{}", name);
            }
        }
    }

    // Default: --field-name
    format!("--{}", field_name.replace("_", "-"))
}

fn infer_field_type(ty: &Type) -> proc_macro2::TokenStream {
    // Check if it's Option<T>
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return infer_field_type_inner(inner_ty);
                    }
                }
            } else {
                return infer_field_type_inner(ty);
            }
        }
    }

    quote! { workflow_manager_sdk::FieldType::Text }
}

fn infer_field_type_inner(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            match type_name.as_str() {
                "String" => quote! { workflow_manager_sdk::FieldType::Text },
                "PathBuf" => quote! { workflow_manager_sdk::FieldType::FilePath { pattern: None } },
                "usize" | "u32" | "u64" | "i32" | "i64" => {
                    quote! { workflow_manager_sdk::FieldType::Number { min: None, max: None } }
                }
                _ => quote! { workflow_manager_sdk::FieldType::Text },
            }
        } else {
            quote! { workflow_manager_sdk::FieldType::Text }
        }
    } else {
        quote! { workflow_manager_sdk::FieldType::Text }
    }
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

fn extract_default(attrs: &[Attribute]) -> proc_macro2::TokenStream {
    for attr in attrs {
        if attr.path().is_ident("arg") {
            let mut default_value = None;

            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("default_value") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        default_value = Some(s.value());
                    }
                }
                Ok(())
            });

            if let Some(val) = default_value {
                return quote! { Some(#val.to_string()) };
            }
        }
    }

    quote! { None }
}
