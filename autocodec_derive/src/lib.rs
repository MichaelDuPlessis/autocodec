use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, Lit, Meta, parse_macro_input};

/// Derive macro that generates `Codec` trait implementations.
///
/// # Structs
///
/// Fields are encoded/decoded sequentially in declaration order.
///
/// ```ignore
/// #[derive(Codec)]
/// struct Header {
///     version: u16,
///     length: u32,
/// }
/// ```
///
/// # Enums
///
/// A `u8` discriminant (variant index starting at 0) is written first, followed by
/// the variant's fields. Maximum 256 variants.
///
/// ```ignore
/// #[derive(Codec)]
/// enum Command {
///     Ping,           // disc 0, no payload
///     Send(Vec<u8>),  // disc 1, tuple variant
///     Move { x: i32, y: i32 }, // disc 2, struct variant
/// }
/// ```
///
/// # Attributes
///
/// - `#[codec(endian = "little")]` — encode/decode this field as little-endian
/// - `#[codec(endian = "big")]` — explicitly mark as big-endian (the default)
#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = match &input.data {
        Data::Struct(data) => impl_struct(name, &impl_generics, &ty_generics, where_clause, &data.fields),
        Data::Enum(data) => impl_enum(name, &impl_generics, &ty_generics, where_clause, data),
        Data::Union(_) => {
            return syn::Error::new_spanned(&input.ident, "Codec cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    expanded.into()
}

#[derive(Clone, Copy, PartialEq)]
enum Endian {
    Big,
    Little,
}

#[derive(Clone, Copy)]
enum LenType {
    U8,
    U16,
    U32,
    U64,
}

struct FieldAttrs {
    endian: Option<Endian>,
    len: Option<LenType>,
    min_len: Option<usize>,
}

fn parse_field_attrs(field: &Field) -> FieldAttrs {
    let mut endian = None;
    let mut len = None;
    let mut min_len = None;
    for attr in &field.attrs {
        if !attr.path().is_ident("codec") {
            continue;
        }
        if let Ok(Meta::NameValue(nv)) = attr.parse_args::<Meta>() {
            if nv.path.is_ident("endian")
                && let syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value
            {
                endian = match s.value().as_str() {
                    "little" => Some(Endian::Little),
                    "big" => Some(Endian::Big),
                    _ => None,
                };
            }
            if nv.path.is_ident("len")
                && let syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value
            {
                len = match s.value().as_str() {
                    "u8" => Some(LenType::U8),
                    "u16" => Some(LenType::U16),
                    "u32" => Some(LenType::U32),
                    "u64" => Some(LenType::U64),
                    _ => None,
                };
            }
            if nv.path.is_ident("min_len")
                && let syn::Expr::Lit(syn::ExprLit { lit: Lit::Int(i), .. }) = &nv.value
            {
                min_len = i.base10_parse::<usize>().ok();
            }
        }
    }
    FieldAttrs { endian, len, min_len }
}

fn len_type_tokens(lt: LenType) -> proc_macro2::TokenStream {
    match lt {
        LenType::U8 => quote! { u8 },
        LenType::U16 => quote! { u16 },
        LenType::U32 => quote! { u32 },
        LenType::U64 => quote! { u64 },
    }
}

fn decode_expr(ty: &syn::Type, attrs: &FieldAttrs) -> proc_macro2::TokenStream {
    if let Some(lt) = attrs.len {
        let len_ty = len_type_tokens(lt);
        return quote! { autocodec::decode_with_len::<#len_ty, #ty>(input)? };
    }
    match attrs.endian {
        Some(Endian::Big) => quote! { autocodec::decode_be::<#ty>(input)? },
        Some(Endian::Little) => quote! { autocodec::decode_le::<#ty>(input)? },
        None => quote! { <#ty as autocodec::Codec>::decode(input)? },
    }
}

fn encode_expr(field_expr: proc_macro2::TokenStream, attrs: &FieldAttrs) -> proc_macro2::TokenStream {
    if let Some(lt) = attrs.len {
        let len_ty = len_type_tokens(lt);
        return quote! { autocodec::encode_with_len::<#len_ty, _>(&#field_expr, buf); };
    }
    match attrs.endian {
        Some(Endian::Big) => quote! { autocodec::encode_be(&#field_expr, buf); },
        Some(Endian::Little) => quote! { autocodec::encode_le(&#field_expr, buf); },
        None => quote! { autocodec::Codec::encode(&#field_expr, buf); },
    }
}

/// Like `encode_expr` but for values that are already references (enum pattern bindings).
fn encode_expr_ref(field_expr: proc_macro2::TokenStream, attrs: &FieldAttrs) -> proc_macro2::TokenStream {
    if let Some(lt) = attrs.len {
        let len_ty = len_type_tokens(lt);
        return quote! { autocodec::encode_with_len::<#len_ty, _>(#field_expr, buf); };
    }
    match attrs.endian {
        Some(Endian::Big) => quote! { autocodec::encode_be(#field_expr, buf); },
        Some(Endian::Little) => quote! { autocodec::encode_le(#field_expr, buf); },
        None => quote! { autocodec::Codec::encode(#field_expr, buf); },
    }
}

fn decode_stmt(binding: proc_macro2::TokenStream, ty: &syn::Type, attrs: &FieldAttrs) -> proc_macro2::TokenStream {
    let expr = decode_expr(ty, attrs);
    let base = quote! { let (#binding, input) = #expr; };
    if let Some(min) = attrs.min_len {
        quote! {
            #base
            autocodec::check_min_len(&#binding, #min)?;
        }
    } else {
        base
    }
}

fn impl_struct(
    name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    fields: &Fields,
) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(f) => {
            let field_names: Vec<_> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
            let field_types: Vec<_> = f.named.iter().map(|f| &f.ty).collect();
            let attrs: Vec<_> = f.named.iter().map(parse_field_attrs).collect();

            let decode_stmts = field_names.iter().zip(field_types.iter()).zip(attrs.iter()).map(|((n, t), a)| {
                decode_stmt(quote! { #n }, t, a)
            });

            let encode_stmts = field_names.iter().zip(attrs.iter()).map(|(n, a)| {
                encode_expr(quote! { self.#n }, a)
            });

            let construct = quote! { Self { #(#field_names),* } };

            quote! {
                impl #impl_generics autocodec::Codec for #name #ty_generics #where_clause {
                    fn decode(input: &[u8]) -> Result<(Self, &[u8]), autocodec::CodecError> {
                        #(#decode_stmts)*
                        Ok((#construct, input))
                    }
                    fn encode(&self, buf: &mut Vec<u8>) {
                        #(#encode_stmts)*
                    }
                }
            }
        }
        Fields::Unnamed(f) => {
            let field_types: Vec<_> = f.unnamed.iter().map(|f| &f.ty).collect();
            let attrs: Vec<_> = f.unnamed.iter().map(parse_field_attrs).collect();
            let field_idents: Vec<_> = (0..field_types.len())
                .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                .collect();

            let decode_stmts = field_idents.iter().zip(field_types.iter()).zip(attrs.iter()).map(|((id, t), a)| {
                decode_stmt(quote! { #id }, t, a)
            });

            let encode_stmts = field_idents.iter().enumerate().zip(attrs.iter()).map(|((i, _), a)| {
                let idx = syn::Index::from(i);
                encode_expr(quote! { self.#idx }, a)
            });

            quote! {
                impl #impl_generics autocodec::Codec for #name #ty_generics #where_clause {
                    fn decode(input: &[u8]) -> Result<(Self, &[u8]), autocodec::CodecError> {
                        #(#decode_stmts)*
                        Ok((Self(#(#field_idents),*), input))
                    }
                    fn encode(&self, buf: &mut Vec<u8>) {
                        #(#encode_stmts)*
                    }
                }
            }
        }
        Fields::Unit => {
            quote! {
                impl #impl_generics autocodec::Codec for #name #ty_generics #where_clause {
                    fn decode(input: &[u8]) -> Result<(Self, &[u8]), autocodec::CodecError> {
                        Ok((Self, input))
                    }
                    fn encode(&self, _buf: &mut Vec<u8>) {}
                }
            }
        }
    }
}

fn impl_enum(
    name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    data: &syn::DataEnum,
) -> proc_macro2::TokenStream {
    if data.variants.len() > 256 {
        return syn::Error::new_spanned(name, "Codec: enums with more than 256 variants are not supported")
            .to_compile_error();
    }

    let decode_arms: Vec<_> = data.variants.iter().enumerate().map(|(i, v)| {
        let disc = i as u8;
        let vname = &v.ident;
        match &v.fields {
            Fields::Unit => quote! { #disc => Ok((Self::#vname, input)), },
            Fields::Named(f) => {
                let field_names: Vec<_> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let field_types: Vec<_> = f.named.iter().map(|f| &f.ty).collect();
                let attrs: Vec<_> = f.named.iter().map(parse_field_attrs).collect();
                let stmts = field_names.iter().zip(field_types.iter()).zip(attrs.iter()).map(|((n, t), a)| {
                    decode_stmt(quote! { #n }, t, a)
                });
                quote! {
                    #disc => {
                        #(#stmts)*
                        Ok((Self::#vname { #(#field_names),* }, input))
                    }
                }
            }
            Fields::Unnamed(f) => {
                let field_types: Vec<_> = f.unnamed.iter().map(|f| &f.ty).collect();
                let attrs: Vec<_> = f.unnamed.iter().map(parse_field_attrs).collect();
                let field_idents: Vec<_> = (0..field_types.len())
                    .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                    .collect();
                let stmts = field_idents.iter().zip(field_types.iter()).zip(attrs.iter()).map(|((id, t), a)| {
                    decode_stmt(quote! { #id }, t, a)
                });
                quote! {
                    #disc => {
                        #(#stmts)*
                        Ok((Self::#vname(#(#field_idents),*), input))
                    }
                }
            }
        }
    }).collect();

    let encode_arms: Vec<_> = data.variants.iter().enumerate().map(|(i, v)| {
        let disc = i as u8;
        let vname = &v.ident;
        match &v.fields {
            Fields::Unit => quote! {
                Self::#vname => { autocodec::Codec::encode(&#disc, buf); }
            },
            Fields::Named(f) => {
                let field_names: Vec<_> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let attrs: Vec<_> = f.named.iter().map(parse_field_attrs).collect();
                let stmts = field_names.iter().zip(attrs.iter()).map(|(n, a)| {
                    encode_expr_ref(quote! { #n }, a)
                });
                quote! {
                    Self::#vname { #(#field_names),* } => {
                        autocodec::Codec::encode(&#disc, buf);
                        #(#stmts)*
                    }
                }
            }
            Fields::Unnamed(f) => {
                let attrs: Vec<_> = f.unnamed.iter().map(parse_field_attrs).collect();
                let field_idents: Vec<_> = (0..f.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                    .collect();
                let stmts = field_idents.iter().zip(attrs.iter()).map(|(id, a)| {
                    encode_expr_ref(quote! { #id }, a)
                });
                quote! {
                    Self::#vname(#(#field_idents),*) => {
                        autocodec::Codec::encode(&#disc, buf);
                        #(#stmts)*
                    }
                }
            }
        }
    }).collect();

    quote! {
        impl #impl_generics autocodec::Codec for #name #ty_generics #where_clause {
            fn decode(input: &[u8]) -> Result<(Self, &[u8]), autocodec::CodecError> {
                let (disc, input) = <u8 as autocodec::Codec>::decode(input)?;
                match disc {
                    #(#decode_arms)*
                    other => Err(autocodec::CodecError::UnknownDiscriminant { value: other }),
                }
            }
            fn encode(&self, buf: &mut Vec<u8>) {
                match self {
                    #(#encode_arms)*
                }
            }
        }
    }
}
