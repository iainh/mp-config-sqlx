use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, LitStr, Type, parse_macro_input};

/// Derives an async `connect` constructor for structs containing SQLx pool fields.
///
/// Supported field types are `PgPool`, `MySqlPool`, and `SqlitePool`, including
/// qualified paths such as `sqlx::PgPool`.
#[proc_macro_derive(Datasources, attributes(datasources, datasource))]
pub fn derive_datasources(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(expand_datasources(input))
}

fn expand_datasources(input: DeriveInput) -> TokenStream2 {
    match try_expand_datasources(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn try_expand_datasources(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let struct_attr = parse_struct_attrs(&input.attrs)?;
    let prefix = struct_attr
        .prefix
        .unwrap_or_else(|| "datasource".to_owned());

    let Data::Struct(data_struct) = input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "Datasources can only be derived for structs",
        ));
    };

    let Fields::Named(fields) = data_struct.fields else {
        return Err(syn::Error::new_spanned(
            ident,
            "Datasources requires a struct with named fields",
        ));
    };

    let initializers = fields
        .named
        .iter()
        .map(expand_field)
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub async fn connect(
                config: &::mp_config::Config,
            ) -> ::mp_config_sqlx::Result<Self> {
                let __mp_config_sqlx_datasources =
                    ::mp_config_sqlx::DatasourcesConfig::from_config_prefix(config, #prefix)?;

                ::std::result::Result::Ok(Self {
                    #(#initializers)*
                })
            }
        }
    })
}

#[derive(Default)]
struct StructAttr {
    prefix: Option<String>,
}

#[derive(Default)]
struct FieldAttr {
    default: bool,
    name: Option<String>,
}

#[derive(Clone, Copy)]
enum PoolKind {
    Postgres,
    MySql,
    Sqlite,
}

fn parse_struct_attrs(attrs: &[Attribute]) -> syn::Result<StructAttr> {
    let mut out = StructAttr::default();
    let mut seen_prefix = false;

    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("datasources"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                reject_duplicate(&mut seen_prefix, "prefix", &meta.path)?;
                let value = meta.value()?;
                out.prefix = Some(value.parse::<LitStr>()?.value());
                Ok(())
            } else {
                Err(meta.error("expected `prefix = \"...\"`"))
            }
        })?;
    }

    Ok(out)
}

fn parse_field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttr> {
    let mut out = FieldAttr::default();
    let mut seen_default = false;
    let mut seen_name = false;

    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("datasource"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                reject_duplicate(&mut seen_default, "default", &meta.path)?;
                out.default = true;
                Ok(())
            } else if meta.path.is_ident("name") {
                reject_duplicate(&mut seen_name, "name", &meta.path)?;
                let value = meta.value()?;
                out.name = Some(value.parse::<LitStr>()?.value());
                Ok(())
            } else {
                Err(meta.error("expected `default` or `name = \"...\"`"))
            }
        })?;
    }

    if out.default && out.name.is_some() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`default` and `name = \"...\"` cannot be used together",
        ));
    }

    Ok(out)
}

fn reject_duplicate(seen: &mut bool, name: &str, path: &syn::Path) -> syn::Result<()> {
    if *seen {
        return Err(syn::Error::new_spanned(
            path,
            format!("duplicate `{name}` argument"),
        ));
    }
    *seen = true;
    Ok(())
}

fn expand_field(field: &syn::Field) -> syn::Result<TokenStream2> {
    let Some(field_ident) = field.ident.as_ref() else {
        return Err(syn::Error::new_spanned(
            field,
            "Datasources requires named fields",
        ));
    };
    let attrs = parse_field_attrs(&field.attrs)?;
    let pool_kind = pool_kind(&field.ty)?;
    let selector = selector_tokens(field_ident, attrs, pool_kind);

    Ok(quote! {
        #field_ident: #selector.connect().await?,
    })
}

fn selector_tokens(field_ident: &Ident, attrs: FieldAttr, pool_kind: PoolKind) -> TokenStream2 {
    match (attrs.default, attrs.name) {
        (true, _) => default_selector(pool_kind),
        (false, Some(name)) => named_selector(pool_kind, quote!(#name)),
        (false, None) => {
            let name = field_ident.to_string();
            named_selector(pool_kind, quote!(#name))
        }
    }
}

fn default_selector(pool_kind: PoolKind) -> TokenStream2 {
    match pool_kind {
        PoolKind::Postgres => quote!(__mp_config_sqlx_datasources.default_postgres()?),
        PoolKind::MySql => quote!(__mp_config_sqlx_datasources.default_mysql()?),
        PoolKind::Sqlite => quote!(__mp_config_sqlx_datasources.default_sqlite()?),
    }
}

fn named_selector(pool_kind: PoolKind, name: TokenStream2) -> TokenStream2 {
    match pool_kind {
        PoolKind::Postgres => quote!(__mp_config_sqlx_datasources.postgres(#name)?),
        PoolKind::MySql => quote!(__mp_config_sqlx_datasources.mysql(#name)?),
        PoolKind::Sqlite => quote!(__mp_config_sqlx_datasources.sqlite(#name)?),
    }
}

fn pool_kind(ty: &Type) -> syn::Result<PoolKind> {
    let Type::Path(type_path) = ty else {
        return Err(unsupported_pool_type(ty));
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Err(unsupported_pool_type(ty));
    };

    match segment.ident.to_string().as_str() {
        "PgPool" => Ok(PoolKind::Postgres),
        "MySqlPool" => Ok(PoolKind::MySql),
        "SqlitePool" => Ok(PoolKind::Sqlite),
        _ => Err(unsupported_pool_type(ty)),
    }
}

fn unsupported_pool_type(ty: &Type) -> syn::Error {
    syn::Error::new_spanned(
        ty,
        "Datasources only supports PgPool, MySqlPool, and SqlitePool fields",
    )
}
