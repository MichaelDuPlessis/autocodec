use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Expr, ExprLit, Field, Fields, Lit, Meta, parse_macro_input};

/// Derive macro that generates `Codec` trait implementations.
///
/// # Attributes
///
/// ## Container-level (on the struct/enum)
///
/// - `#[codec(endian = "little")]` — set default endianness for all integer fields
///
/// ## Field-level
///
/// - `#[codec(endian = "little")]` / `#[codec(endian = "big")]` — per-field endianness
/// - `#[codec(len = "u8")]` — custom length prefix type for Vec/String
/// - `#[codec(min_len = N)]` — minimum length constraint
/// - `#[codec(max_len = N)]` — maximum length constraint
/// - `#[codec(skip)]` — skip field in wire format, use Default on decode
/// - `#[codec(padding = N)]` — insert N zero bytes after this field
/// - `#[codec(magic = 0xNN)]` — expect a constant value, error on mismatch
/// - `#[codec(validate = "fn_name")]` — call a validation function after decode
///
/// ## Enum-level
///
/// - `#[codec(discriminant_type = "u16")]` — wider discriminant (default u8)
/// - `#[codec(discriminant = N)]` on variants — custom discriminant value
#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let container_attrs = parse_container_attrs(&input);

    let expanded = match &input.data {
        Data::Struct(data) => impl_struct(name, &impl_generics, &ty_generics, where_clause, &data.fields, &container_attrs),
        Data::Enum(data) => impl_enum(name, &impl_generics, &ty_generics, where_clause, data, &container_attrs),
        Data::Union(_) => {
            return syn::Error::new_spanned(&input.ident, "Codec cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    expanded.into()
}

// --- Attribute types ---

#[derive(Clone, Copy, PartialEq)]
enum Endian { Big, Little }

#[derive(Clone, Copy)]
enum LenType { U8, U16, U32, U64 }

#[derive(Clone, Copy)]
enum DiscType { U8, U16, U32 }

struct ContainerAttrs {
    endian: Option<Endian>,
    disc_type: DiscType,
}

struct FieldAttrs {
    endian: Option<Endian>,
    len: Option<LenType>,
    min_len: Option<usize>,
    max_len: Option<usize>,
    skip: bool,
    padding: Option<usize>,
    magic: Option<u64>,
    validate: Option<String>,
    with_module: Option<String>,
    default_expr: Option<String>,
}

fn parse_container_attrs(input: &DeriveInput) -> ContainerAttrs {
    let mut endian = None;
    let mut disc_type = DiscType::U8;
    for attr in &input.attrs {
        if !attr.path().is_ident("codec") { continue; }
        if let Ok(Meta::NameValue(nv)) = attr.parse_args::<Meta>() {
            if nv.path.is_ident("endian")
                && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
            {
                endian = match s.value().as_str() {
                    "little" => Some(Endian::Little),
                    "big" => Some(Endian::Big),
                    _ => None,
                };
            }
            if nv.path.is_ident("discriminant_type")
                && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
            {
                disc_type = match s.value().as_str() {
                    "u8" => DiscType::U8,
                    "u16" => DiscType::U16,
                    "u32" => DiscType::U32,
                    _ => DiscType::U8,
                };
            }
        }
    }
    ContainerAttrs { endian, disc_type }
}

fn parse_field_attrs(field: &Field) -> FieldAttrs {
    let mut attrs = FieldAttrs {
        endian: None, len: None, min_len: None, max_len: None,
        skip: false, padding: None, magic: None, validate: None,
        with_module: None, default_expr: None,
    };
    for attr in &field.attrs {
        if !attr.path().is_ident("codec") { continue; }
        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
        );
        let Ok(nested) = nested else { continue; };
        for meta in nested {
            match meta {
                Meta::Path(p) if p.is_ident("skip") => {
                    attrs.skip = true;
                }
                Meta::NameValue(nv) => {
                    if nv.path.is_ident("endian")
                        && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
                    {
                        attrs.endian = match s.value().as_str() {
                            "little" => Some(Endian::Little),
                            "big" => Some(Endian::Big),
                            _ => None,
                        };
                    }
                    if nv.path.is_ident("len")
                        && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
                    {
                        attrs.len = match s.value().as_str() {
                            "u8" => Some(LenType::U8),
                            "u16" => Some(LenType::U16),
                            "u32" => Some(LenType::U32),
                            "u64" => Some(LenType::U64),
                            _ => None,
                        };
                    }
                    if nv.path.is_ident("min_len")
                        && let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = &nv.value
                    {
                        attrs.min_len = i.base10_parse().ok();
                    }
                    if nv.path.is_ident("max_len")
                        && let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = &nv.value
                    {
                        attrs.max_len = i.base10_parse().ok();
                    }
                    if nv.path.is_ident("padding")
                        && let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = &nv.value
                    {
                        attrs.padding = i.base10_parse().ok();
                    }
                    if nv.path.is_ident("magic")
                        && let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = &nv.value
                    {
                        attrs.magic = i.base10_parse().ok();
                    }
                    if nv.path.is_ident("validate")
                        && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
                    {
                        attrs.validate = Some(s.value());
                    }
                    if nv.path.is_ident("with")
                        && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
                    {
                        attrs.with_module = Some(s.value());
                    }
                    if nv.path.is_ident("default")
                        && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
                    {
                        attrs.default_expr = Some(s.value());
                    }
                }
                _ => {}
            }
        }
    }
    attrs
}

fn parse_variant_discriminant(v: &syn::Variant) -> Option<u64> {
    for attr in &v.attrs {
        if !attr.path().is_ident("codec") { continue; }
        if let Ok(Meta::NameValue(nv)) = attr.parse_args::<Meta>()
            && nv.path.is_ident("discriminant")
            && let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = &nv.value
        {
            return i.base10_parse().ok();
        }
    }
    None
}

// --- Code generation helpers ---

fn len_type_tokens(lt: LenType) -> proc_macro2::TokenStream {
    match lt {
        LenType::U8 => quote! { u8 },
        LenType::U16 => quote! { u16 },
        LenType::U32 => quote! { u32 },
        LenType::U64 => quote! { u64 },
    }
}

fn effective_endian(field: &FieldAttrs, container: &ContainerAttrs) -> Option<Endian> {
    field.endian.or(container.endian)
}

fn decode_expr(ty: &syn::Type, attrs: &FieldAttrs, container: &ContainerAttrs) -> proc_macro2::TokenStream {
    if let Some(lt) = attrs.len {
        let len_ty = len_type_tokens(lt);
        return quote! { autocodec::decode_with_len::<#len_ty, #ty>(input)? };
    }
    match effective_endian(attrs, container) {
        Some(Endian::Big) => quote! { autocodec::decode_be::<#ty>(input)? },
        Some(Endian::Little) => quote! { autocodec::decode_le::<#ty>(input)? },
        None => quote! { <#ty as autocodec::Codec>::decode(input)? },
    }
}

fn encode_expr(field_expr: proc_macro2::TokenStream, attrs: &FieldAttrs, container: &ContainerAttrs) -> proc_macro2::TokenStream {
    if let Some(lt) = attrs.len {
        let len_ty = len_type_tokens(lt);
        return quote! { autocodec::encode_with_len::<#len_ty, _>(&#field_expr, buf); };
    }
    match effective_endian(attrs, container) {
        Some(Endian::Big) => quote! { autocodec::encode_be(&#field_expr, buf); },
        Some(Endian::Little) => quote! { autocodec::encode_le(&#field_expr, buf); },
        None => quote! { autocodec::Codec::encode(&#field_expr, buf); },
    }
}

fn encode_expr_ref(field_expr: proc_macro2::TokenStream, attrs: &FieldAttrs, container: &ContainerAttrs) -> proc_macro2::TokenStream {
    if let Some(lt) = attrs.len {
        let len_ty = len_type_tokens(lt);
        return quote! { autocodec::encode_with_len::<#len_ty, _>(#field_expr, buf); };
    }
    match effective_endian(attrs, container) {
        Some(Endian::Big) => quote! { autocodec::encode_be(#field_expr, buf); },
        Some(Endian::Little) => quote! { autocodec::encode_le(#field_expr, buf); },
        None => quote! { autocodec::Codec::encode(#field_expr, buf); },
    }
}

fn decode_stmt(binding: proc_macro2::TokenStream, ty: &syn::Type, attrs: &FieldAttrs, container: &ContainerAttrs) -> proc_macro2::TokenStream {
    // Magic field: decode and validate constant, bind default
    if let Some(magic) = attrs.magic {
        let magic_lit = syn::LitInt::new(&format!("{magic}"), proc_macro2::Span::call_site());
        return quote! {
            let input = autocodec::decode_magic_u32(input, #magic_lit as u32)?;
            let #binding = <#ty as Default>::default();
        };
    }

    // Skip field with optional custom default
    if attrs.skip {
        if let Some(ref expr_str) = attrs.default_expr {
            let expr: syn::Expr = syn::parse_str(expr_str).expect("invalid default expression");
            return quote! { let #binding = #expr; };
        }
        return quote! { let #binding = autocodec::skip_decode::<#ty>(); };
    }

    // Custom with module
    if let Some(ref module) = attrs.with_module {
        let mod_path: syn::Path = syn::parse_str(module).expect("invalid with module path");
        let mut stmts = quote! { let (#binding, input) = #mod_path::decode(input)?; };
        if let Some(pad) = attrs.padding {
            stmts = quote! { #stmts let input = autocodec::decode_padding(input, #pad)?; };
        }
        return stmts;
    }

    let expr = decode_expr(ty, attrs, container);
    let mut stmts = quote! { let (#binding, input) = #expr; };

    if let Some(min) = attrs.min_len {
        stmts = quote! { #stmts autocodec::check_min_len(&#binding, #min)?; };
    }
    if let Some(max) = attrs.max_len {
        stmts = quote! { #stmts autocodec::check_max_len(&#binding, #max)?; };
    }
    if let Some(ref func) = attrs.validate {
        let func_ident = syn::Ident::new(func, proc_macro2::Span::call_site());
        stmts = quote! {
            #stmts
            if !#func_ident(&#binding) {
                return Err(autocodec::CodecError::ValidationFailed);
            }
        };
    }
    if let Some(pad) = attrs.padding {
        stmts = quote! { #stmts let input = autocodec::decode_padding(input, #pad)?; };
    }

    stmts
}

fn encode_field_stmt(field_expr: proc_macro2::TokenStream, attrs: &FieldAttrs, container: &ContainerAttrs, is_ref: bool) -> proc_macro2::TokenStream {
    if let Some(magic) = attrs.magic {
        let magic_lit = syn::LitInt::new(&format!("{magic}u32"), proc_macro2::Span::call_site());
        let mut s = quote! { autocodec::Codec::encode(&#magic_lit, buf); };
        if let Some(pad) = attrs.padding {
            s = quote! { #s autocodec::encode_padding(buf, #pad); };
        }
        return s;
    }
    if attrs.skip {
        return if let Some(pad) = attrs.padding {
            quote! { autocodec::encode_padding(buf, #pad); }
        } else {
            quote! {}
        };
    }

    if let Some(ref module) = attrs.with_module {
        let mod_path: syn::Path = syn::parse_str(module).expect("invalid with module path");
        let mut s = if is_ref {
            quote! { #mod_path::encode(#field_expr, buf); }
        } else {
            quote! { #mod_path::encode(&#field_expr, buf); }
        };
        if let Some(pad) = attrs.padding {
            s = quote! { #s autocodec::encode_padding(buf, #pad); };
        }
        return s;
    }

    let mut s = if is_ref {
        encode_expr_ref(field_expr, attrs, container)
    } else {
        encode_expr(field_expr, attrs, container)
    };

    if let Some(pad) = attrs.padding {
        s = quote! { #s autocodec::encode_padding(buf, #pad); };
    }
    s
}

// --- Struct impl ---

fn impl_struct(
    name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    fields: &Fields,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(f) => {
            let field_names: Vec<_> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
            let field_types: Vec<_> = f.named.iter().map(|f| &f.ty).collect();
            let attrs: Vec<_> = f.named.iter().map(parse_field_attrs).collect();

            let decode_stmts = field_names.iter().zip(field_types.iter()).zip(attrs.iter()).map(|((n, t), a)| {
                decode_stmt(quote! { #n }, t, a, container)
            });
            let encode_stmts = field_names.iter().zip(attrs.iter()).map(|(n, a)| {
                encode_field_stmt(quote! { self.#n }, a, container, false)
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
                decode_stmt(quote! { #id }, t, a, container)
            });
            let encode_stmts = field_idents.iter().enumerate().zip(attrs.iter()).map(|((i, _), a)| {
                let idx = syn::Index::from(i);
                encode_field_stmt(quote! { self.#idx }, a, container, false)
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

// --- Enum impl ---

fn disc_type_tokens(dt: DiscType) -> proc_macro2::TokenStream {
    match dt {
        DiscType::U8 => quote! { u8 },
        DiscType::U16 => quote! { u16 },
        DiscType::U32 => quote! { u32 },
    }
}

fn disc_max(dt: DiscType) -> u64 {
    match dt {
        DiscType::U8 => 255,
        DiscType::U16 => 65535,
        DiscType::U32 => u32::MAX as u64,
    }
}

fn impl_enum(
    name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    data: &syn::DataEnum,
    container: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    let dt = container.disc_type;
    let disc_ty = disc_type_tokens(dt);
    let max_disc = disc_max(dt);

    // Compute discriminant values
    let disc_values: Vec<u64> = {
        let mut vals = Vec::new();
        let mut next = 0u64;
        for v in &data.variants {
            let d = parse_variant_discriminant(v).unwrap_or(next);
            vals.push(d);
            next = d + 1;
        }
        vals
    };

    // Check discriminants fit
    for (i, &d) in disc_values.iter().enumerate() {
        if d > max_disc {
            let v = &data.variants.iter().nth(i).unwrap().ident;
            return syn::Error::new_spanned(v,
                format!("Codec: discriminant {d} exceeds maximum for {}", quote!(#disc_ty)))
                .to_compile_error();
        }
    }

    let decode_arms: Vec<_> = data.variants.iter().zip(disc_values.iter()).map(|(v, &disc)| {
        let vname = &v.ident;
        let disc_lit = syn::LitInt::new(&format!("{disc}"), proc_macro2::Span::call_site());
        match &v.fields {
            Fields::Unit => quote! { #disc_lit => Ok((Self::#vname, input)), },
            Fields::Named(f) => {
                let field_names: Vec<_> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let field_types: Vec<_> = f.named.iter().map(|f| &f.ty).collect();
                let attrs: Vec<_> = f.named.iter().map(parse_field_attrs).collect();
                let stmts = field_names.iter().zip(field_types.iter()).zip(attrs.iter()).map(|((n, t), a)| {
                    decode_stmt(quote! { #n }, t, a, container)
                });
                quote! {
                    #disc_lit => {
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
                    decode_stmt(quote! { #id }, t, a, container)
                });
                quote! {
                    #disc_lit => {
                        #(#stmts)*
                        Ok((Self::#vname(#(#field_idents),*), input))
                    }
                }
            }
        }
    }).collect();

    let encode_arms: Vec<_> = data.variants.iter().zip(disc_values.iter()).map(|(v, &disc)| {
        let vname = &v.ident;
        let disc_lit = syn::LitInt::new(&format!("{disc}"), proc_macro2::Span::call_site());
        match &v.fields {
            Fields::Unit => quote! {
                Self::#vname => { autocodec::Codec::encode(&(#disc_lit as #disc_ty), buf); }
            },
            Fields::Named(f) => {
                let field_names: Vec<_> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let attrs: Vec<_> = f.named.iter().map(parse_field_attrs).collect();
                let stmts = field_names.iter().zip(attrs.iter()).map(|(n, a)| {
                    encode_field_stmt(quote! { #n }, a, container, true)
                });
                quote! {
                    Self::#vname { #(#field_names),* } => {
                        autocodec::Codec::encode(&(#disc_lit as #disc_ty), buf);
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
                    encode_field_stmt(quote! { #id }, a, container, true)
                });
                quote! {
                    Self::#vname(#(#field_idents),*) => {
                        autocodec::Codec::encode(&(#disc_lit as #disc_ty), buf);
                        #(#stmts)*
                    }
                }
            }
        }
    }).collect();

    quote! {
        impl #impl_generics autocodec::Codec for #name #ty_generics #where_clause {
            fn decode(input: &[u8]) -> Result<(Self, &[u8]), autocodec::CodecError> {
                let (disc, input) = <#disc_ty as autocodec::Codec>::decode(input)?;
                match disc {
                    #(#decode_arms)*
                    _ => Err(autocodec::CodecError::UnknownDiscriminant { value: disc as u8 }),
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
