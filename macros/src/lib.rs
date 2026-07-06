use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro_error::proc_macro_error;
use proc_macro2::Span;
use quote::{ToTokens as _, quote};
use syn::{
    Data, DeriveInput, Expr, ExprLit, Ident, LitStr, MetaNameValue, Type, spanned::Spanned as _,
};

extern crate proc_macro;

struct Field {
    ident: Ident,
    ty: Type,
    name: LitStr,
}

fn field_name(ident: &Ident, attrs: &[syn::Attribute]) -> LitStr {
    for attr in attrs {
        if let syn::Meta::NameValue(MetaNameValue { path, value, .. }) = &attr.meta
            && let Some(key) = path.get_ident()
            && key.to_string() == "name"
        {
            return if let Expr::Lit(ExprLit { lit, .. }) = value
                && let syn::Lit::Str(x) = &lit
            {
                x.clone()
            } else {
                proc_macro_error::abort!(value.span(), "name must be a string literal")
            };
        }
    }
    LitStr::new(&ident.to_string(), Span::call_site())
}

impl Field {
    fn from_syn(
        syn::Field {
            attrs, ident, ty, ..
        }: &syn::Field,
    ) -> Self {
        let ident = ident
            .clone()
            .unwrap_or_else(|| proc_macro_error::abort!(ident.span(), "field must have a name"));
        Field {
            ty: ty.clone(),
            name: field_name(&ident, attrs),
            ident,
        }
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn dict(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as DeriveInput);
    let dict = match &mut input.data {
        Data::Struct(s) => s,
        _ => proc_macro_error::abort_call_site!("`derive(Dict)` should be used on a struct"),
    };
    for field in &mut dict.fields {
        let ty = &field.ty;
        field.ty = syn::parse2(quote!(Option<#ty>)).unwrap()
    }
    input.into_token_stream().into()
}

#[proc_macro_error]
#[proc_macro_derive(Dict, attributes(name))]
pub fn impl_dict(input: TokenStream) -> TokenStream {
    let crate_path = match crate_name("dbus-marshal") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => proc_macro_error::abort_call_site!("dbus-marshal not found"),
    };
    let input = syn::parse_macro_input!(input as DeriveInput);

    let mut iter = input.generics.params.iter();
    let unmarshal_lifetime = match (iter.next(), iter.next()) {
        (None, None) => quote!(<'_>),
        (Some(syn::GenericParam::Lifetime(syn::LifetimeParam { lifetime, .. })), None) => {
            quote!(<#lifetime>)
        }
        _ => proc_macro_error::abort!(
            input.generics.span(),
            "dict must contain no more than 1 lifetime generic parameter"
        ),
    };
    let lifetime = &input.generics;

    let dict = match &input.data {
        Data::Struct(s) => s,
        _ => proc_macro_error::abort_call_site!("`derive(Dict)` should be used on a struct"),
    };
    let fields: Vec<_> = dict.fields.iter().map(Field::from_syn).collect();

    let dict_name = &input.ident;
    let key_name = Ident::new(&format!("{}Key", input.ident), Span::call_site());
    let value_name = Ident::new(&format!("{}Value", input.ident), Span::call_site());
    let entry_name = Ident::new(&format!("{}Entry", input.ident), Span::call_site());
    let key_fields = fields.iter().map(|Field { ident, .. }| ident);
    let value_fields = fields.iter().map(|Field { ident, ty, .. }| {
        quote! {
            #ident: #ty,
        }
    });
    let unmarshal_key = fields.iter().map(|Field { ident, ty, name }| {
        quote! {
            #name => {
                let val: #crate_path::Variant<#ty> = r.read()?;
                Ok(Self(
                    #key_name::#ident,
                    #value_name {
                        #ident: val.0,
                    },
                ))
            }
        }
    });
    let dict_init_fields = key_fields.clone();
    let unmarshal_fields = key_fields.clone();
    let marshal_fields = fields.iter().map(|Field { ident, name, .. }| {
        quote! {
            if let Some(value) = self.#ident {
                w.align_to(8);
                w.write(#name);
                w.write(&#crate_path::Variant(value));
            }
        }
    });

    // TODO: allow unknown fields
    quote! {
        impl #lifetime #crate_path::signature::SignatureProxy for #dict_name #lifetime {
            type Proxy<'_a> = [#crate_path::Entry<&'static str, #crate_path::Variant>];
        }
        impl #lifetime #crate_path::unmarshal::Unmarshal #unmarshal_lifetime for #dict_name #lifetime {
            fn unmarshal(r: &mut #crate_path::unmarshal::Reader #unmarshal_lifetime) -> #crate_path::unmarshal::Result<Self> {
                #[allow(non_camel_case_types)]
                enum #key_name {
                    #(#key_fields),*
                }
                union #value_name #lifetime {
                    #(#value_fields)*
                }
                struct #entry_name #lifetime(#key_name, #value_name #lifetime);

                impl #lifetime #crate_path::signature::SignatureProxy for #entry_name #lifetime {
                    type Proxy<'_a> = #crate_path::Entry<&'static str, #crate_path::Variant>;
                }
                impl #lifetime #crate_path::unmarshal::Unmarshal #unmarshal_lifetime for #entry_name #lifetime {
                    fn unmarshal(r: &mut #crate_path::unmarshal::Reader #unmarshal_lifetime) -> #crate_path::unmarshal::Result<Self> {
                        let key: &#crate_path::String = r.read()?;
                        match unsafe { str::from_utf8_unchecked(key) } {
                            #(#unmarshal_key)*
                            x => {dbg!(x); Err(#crate_path::unmarshal::Error::InvalidArgs)?}
                        }
                    }
                }
                let mut res = Self { #(#dict_init_fields: None,)* };
                let seq: #crate_path::unmarshal::ArrayIter<'_, #entry_name> = r.read()?;
                for entry in seq {
                    let #entry_name(key, val) = entry?;
                    match key {
                        #(#key_name::#unmarshal_fields => res.#unmarshal_fields = Some(unsafe { val.#unmarshal_fields }),)*
                    }
                }
                Ok(res)
            }
        }
        impl #lifetime #crate_path::marshal::Marshal for #dict_name #lifetime {
            fn marshal<W: #crate_path::marshal::Write + ?Sized>(&self, w: &mut W) {
                let insert_pos = w.skip_aligned(4);
                let begin = w.position();
                #(#marshal_fields)*
                let len = w.position() - begin;
                w.insert(&(len as u32), insert_pos);
            }
        }
    }
    .into()
}
