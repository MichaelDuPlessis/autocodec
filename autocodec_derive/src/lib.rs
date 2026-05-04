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

fn field_endian(field: &Field) -> Option<Endian> {
    for attr in &field.attrs {
        if !attr.path().is_ident("codec") {
            continue;
        }
        if let Ok(Meta::NameValue(nv)) = attr.parse_args::<Meta>()
            && nv.path.is_ident("endian")
            && let syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value
        {
            return match s.value().as_str() {
                "little" => Some(Endian::Little),
                "big" => Some(Endian::Big),
                _ => None,
            };
        }
    }
    None
}

fn decode_expr(ty: &syn::Type, endian: Option<Endian>) -> proc_macro2::TokenStream {
    match endian {
        Some(Endian::Little) => quote! { autocodec::decode_le::<#ty>(input)? },
        _ => quote! { <#ty as autocodec::Codec>::decode(input)? },
    }
}

fn encode_expr(field_expr: proc_macro2::TokenStream, endian: Option<Endian>) -> proc_macro2::TokenStream {
    match endian {
        Some(Endian::Little) => quote! { autocodec::encode_le(&#field_expr, buf); },
        _ => quote! { autocodec::Codec::encode(&#field_expr, buf); },
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
            let endians: Vec<_> = f.named.iter().map(field_endian).collect();

            let decode_stmts = field_names.iter().zip(field_types.iter()).zip(endians.iter()).map(|((n, t), e)| {
                let expr = decode_expr(t, *e);
                quote! { let (#n, input) = #expr; }
            });

            let encode_stmts = field_names.iter().zip(endians.iter()).map(|(n, e)| {
                encode_expr(quote! { self.#n }, *e)
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
            let endians: Vec<_> = f.unnamed.iter().map(field_endian).collect();
            let field_idents: Vec<_> = (0..field_types.len())
                .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                .collect();

            let decode_stmts = field_idents.iter().zip(field_types.iter()).zip(endians.iter()).map(|((id, t), e)| {
                let expr = decode_expr(t, *e);
                quote! { let (#id, input) = #expr; }
            });

            let encode_stmts = field_idents.iter().enumerate().zip(endians.iter()).map(|((i, _), e)| {
                let idx = syn::Index::from(i);
                encode_expr(quote! { self.#idx }, *e)
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
                let endians: Vec<_> = f.named.iter().map(field_endian).collect();
                let stmts = field_names.iter().zip(field_types.iter()).zip(endians.iter()).map(|((n, t), e)| {
                    let expr = decode_expr(t, *e);
                    quote! { let (#n, input) = #expr; }
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
                let endians: Vec<_> = f.unnamed.iter().map(field_endian).collect();
                let field_idents: Vec<_> = (0..field_types.len())
                    .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                    .collect();
                let stmts = field_idents.iter().zip(field_types.iter()).zip(endians.iter()).map(|((id, t), e)| {
                    let expr = decode_expr(t, *e);
                    quote! { let (#id, input) = #expr; }
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
                let endians: Vec<_> = f.named.iter().map(field_endian).collect();
                let stmts = field_names.iter().zip(endians.iter()).map(|(n, e)| {
                    match e {
                        Some(Endian::Little) => quote! { autocodec::encode_le(#n, buf); },
                        _ => quote! { autocodec::Codec::encode(#n, buf); },
                    }
                });
                quote! {
                    Self::#vname { #(#field_names),* } => {
                        autocodec::Codec::encode(&#disc, buf);
                        #(#stmts)*
                    }
                }
            }
            Fields::Unnamed(f) => {
                let endians: Vec<_> = f.unnamed.iter().map(field_endian).collect();
                let field_idents: Vec<_> = (0..f.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                    .collect();
                let stmts = field_idents.iter().zip(endians.iter()).map(|(id, e)| {
                    match e {
                        Some(Endian::Little) => quote! { autocodec::encode_le(#id, buf); },
                        _ => quote! { autocodec::Codec::encode(#id, buf); },
                    }
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
