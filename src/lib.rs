use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident, Lit, Meta, NestedMeta,
    PathArguments, Type, TypePath,
};
fn map_to_type(ty: &Type) -> quote::__private::TokenStream {
    match ty {
        syn::Type::Path(tp) => match tp.path.segments.last().unwrap().ident.to_string().as_str() {
            "i16" => {
                quote! {
                    {
                        inner.as_integer().map(|a| a as i16)
                    }
                }
            }
            "u16" => {
                quote! {
                    {
                        inner.as_integer().map(|a| a as u16)
                    }
                }
            }
            "u32" => {
                quote! {
                    {
                        inner.as_integer().map(|a| a as u32)
                    }
                }
            }
            "i32" => {
                quote! {
                    {
                        inner.as_integer().map(|a| a as i32)
                    }
                }
            }
            "u64" => {
                quote! {
                    {
                        inner.as_integer().map(|a| a as u64)
                    }
                }
            }
            "i64" => {
                quote! {
                    {
                        inner.as_integer().map(|a| a as i64)
                    }
                }
            }
            "bool" => {
                quote! {
                    {
                        inner.as_bool()
                    }
                }
            }
            "f32" => {
                quote! {
                    {
                        inner.as_float().map(|a| a as f32)
                    }
                }
            }
            "f64" => {
                quote! {
                    {
                        inner.as_float().map(|a| a as f64)
                    }
                }
            }
            "Option" => {
                let inner = tp.path.segments.first().unwrap();
                match &inner.arguments {
                    PathArguments::AngleBracketed(args) => {
                        let nt = args.args.first().unwrap();
                        match nt {
                            syn::GenericArgument::Type(new_type) => {
                                let conversion = map_to_type(new_type);
                                quote! {
                                    {
                                        if let etoml::Value::Null = &inner {
                                            Some(None)
                                        } else {
                                            Some(#conversion)
                                        }
                                    }
                                }
                            }
                            _ => unimplemented! {"not implemented"},
                        }
                    }
                    _ => unimplemented! {"we don't implement for weird stuff"},
                }
            }
            "HashMap" => {
                let inner = tp.path.segments.first().unwrap();
                match &inner.arguments {
                    PathArguments::AngleBracketed(args) => {
                        let key_type = &args.args[0];
                        let value_type = &args.args[1];
                        let key_type = match key_type {
                            syn::GenericArgument::Type(new_type) => new_type,
                            _ => {
                                unimplemented!("not implemented")
                            }
                        };
                        let value_type = match value_type {
                            syn::GenericArgument::Type(new_type) => new_type,
                            _ => {
                                unimplemented!("not implemented")
                            }
                        };

                        if let syn::Type::Path(tp) = key_type {
                            if tp.path.segments.first().unwrap().ident.to_string().as_str()
                                != "String"
                            {
                                unimplemented!("Key type has to be string");
                            }
                        }
                        let conversion = map_to_type(value_type);

                        quote! {
                            {
                                let mut the_map = HashMap::new();
                                let mut removed_keys = vec![];
                                if let Some(v) = inner.as_object() {
                                     for (name, inner) in &v {
                                        if let Some(inner_value) = #conversion {
                                            the_map.insert(name.clone(), inner_value);
                                            removed_keys.push(name.clone());
                                        }
                                    }
                                    for k in removed_keys {
                                        inner.take(&k);
                                    }
                                    Some(the_map)
                                } else {
                                    None
                                }

                            }
                        }
                    }
                    _ => unimplemented!("not implemented"),
                }
            }
            "Vec" => {
                let inner = tp.path.segments.first().unwrap();
                match &inner.arguments {
                    PathArguments::AngleBracketed(args) => {
                        let nt = args.args.first().unwrap();
                        match nt {
                            syn::GenericArgument::Type(new_type) => {
                                let conversion = map_to_type(new_type);
                                quote! {
                                    {
                                        inner.as_array().map(|a| a.iter().filter_map(|inner| {
                                            #conversion
                                        }).collect::<Vec<#new_type>>())
                                    }
                                }
                            }
                            _ => unimplemented! {"not implemented"},
                        }
                    }
                    _ => unimplemented! {"we don't implement for weird stuff"},
                }
            }
            "String" => {
                quote! {
                    {
                       inner.as_string()
                    }
                }
            }
            _ => {
                quote! {
                    {
                       if let Ok(val) = #tp::from_value(inner.clone(), global_symbol_table.clone()) {
                           Some(val)
                       } else {
                           None
                       }
                    }
                }
            }
        },
        _ => todo!(),
    }
}
fn get_conversion_from_type(
    parent_ident: &Ident,
    name: &Ident,
    ty: &Type,
    default: quote::__private::TokenStream,
) -> quote::__private::TokenStream {
    let name = name.to_string();
    let the_type = match ty {
        syn::Type::Path(tp) => tp.path.segments.first().unwrap().ident.to_string(),
        _ => "".to_string(),
    };
    let conversion = map_to_type(ty);
    let hash_map_code = if the_type == "HashMap" {
        quote! {
            if let etoml::Value::Null = inner {
                replace = true;
                inner = v.clone();
            }
        }
    } else {
        quote! {}
    };
    let rest_code = if the_type == "Vec" {
        quote! {
            converted_value.unwrap_or(vec![])
        }
    } else {
        default
    };
    quote! {
        {
            let mut inner = #parent_ident.take(#name);
            let mut replace = false;
            #hash_map_code
            let converted_value = #conversion;
            if replace {
                #parent_ident = inner.clone();
            }
            #rest_code
        }
    }
}

#[proc_macro_derive(
    Deserialize,
    attributes(skip, from_parent, type_alias, from_global, default_value)
)]
pub fn derive_etoml(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };
    let field_name: Vec<Ident> = fields
        .iter()
        .filter_map(|field| field.ident.to_owned())
        .collect();
    let field_attributes: Vec<_> = fields.iter().map(|field| &field.attrs).collect();

    let field_type: Vec<Type> = fields.iter().map(|field| field.ty.to_owned()).collect();
    let struct_name = &input.ident;

    let mut from_convs = vec![];

    for (the_ident, (ty, attrs)) in field_name
        .iter()
        .zip(field_type.iter().zip(field_attributes))
    {
        let name = the_ident.to_string();
        if attrs
            .iter()
            .any(|a| a.path.segments.first().unwrap().ident == "skip")
        {
            from_convs.push(quote! {
                None
            });
            continue;
        }
        let ty = if let Some(attr) = attrs
            .iter()
            .find(|a| a.path.segments.first().unwrap().ident == "type_alias")
        {
            let the_input =
                if let Meta::List(meta) = attr.parse_meta().expect("use type_alias(type)") {
                    meta
                } else {
                    panic!("use type_alias(type)")
                };

            let path = if let NestedMeta::Meta(meta) = the_input.nested.last().unwrap() {
                meta.path().to_owned()
            } else {
                panic!("use type_alias(type)")
            };
            Type::Path(TypePath { qself: None, path })
        } else {
            ty.to_owned()
        };
        let default_value = if let Some(attr) = attrs
            .iter()
            .find(|a| a.path.segments.first().unwrap().ident == "default_value")
        {
            match attr
                .parse_meta()
                .expect("use default_value or default_value = \"identifier\"")
            {
                Meta::Path(_) => {
                    quote! {
                        converted_value.unwrap_or_default()
                    }
                }
                Meta::NameValue(name_value) => {
                    let str_ident = if let Lit::Str(str) = name_value.lit {
                        Ident::new(str.value().as_str(), the_ident.span())
                    } else {
                        panic!("Only string literals are allowed");
                    };
                    quote! {
                        converted_value.unwrap_or_else(#str_ident)
                    }
                }
                _ => unreachable!("use default_value or default_value = \"identifier\""),
            }
        } else {
            quote! {
                if let Some(val) = converted_value {
                    val
                } else {
                    if !replace {
                        v.set(#name, inner);
                    }
                    return Err("Wrong type".to_string().into());
                }
            }
        };
        let conv = if attrs
            .iter()
            .any(|a| a.path.segments.first().unwrap().ident == "from_global")
        {
            get_conversion_from_type(
                &Ident::new("global_symbol_table", the_ident.span()),
                the_ident,
                &ty,
                default_value,
            )
        } else {
            get_conversion_from_type(
                &Ident::new("v", the_ident.span()),
                the_ident,
                &ty,
                default_value,
            )
        };

        from_convs.push(conv);
    }
    let expanded = quote! {
        #[allow(clippy::eval_order_dependence)]
        impl #struct_name {
            pub fn from_value(mut v: etoml::Value, mut global_symbol_table: etoml::Value) -> Result<Self, Box<dyn std::error::Error>> {
                let obj = v.as_object().ok_or_else(||format!("structs need to be objects"))?;
                Ok(Self {
                    #(
                        #field_name : #from_convs
                    ,)*
                })
            }

            pub fn from_str(input : &str) -> Result<Self, Box<dyn std::error::Error>> {
                use std::convert::TryFrom;
                let file = etoml::EToml::try_from(input).unwrap();

                let value = etoml::Value::Object(file.tables);
                let global_symbol_table = etoml::Value::Object(file.global_symbols);
                Self::from_value(value, global_symbol_table)
            }
        }
    };
    TokenStream::from(expanded)
}
