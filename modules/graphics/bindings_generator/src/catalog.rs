use crate::{MAX_ARGUMENTS, context::LvglContext, function::get_function_identifier};
use quote::ToTokens;
use std::{collections::BTreeSet, fmt};
use syn::{FnArg, Pat, ReturnType, Signature, Type};

#[derive(Debug)]
pub struct GeneratorError(String);

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for GeneratorError {}

#[derive(Debug)]
pub struct Call {
    pub id: u16,
    pub name: String,
    pub signature: Signature,
    pub argument_count: usize,
    pub host_arguments: Vec<String>,
}

impl Call {
    /// Conservative policy: pointer lifetimes cannot be inferred from a C signature.
    /// Strings, arrays, native structures and unions need explicit adapters.
    pub fn generic_safe(&self) -> bool {
        fn safe_type(ty: &Type) -> bool {
            match ty {
                Type::Path(path) => {
                    let name = path.to_token_stream().to_string();
                    matches!(
                        name.as_str(),
                        "u8" | "u16"
                            | "u32"
                            | "u64"
                            | "i8"
                            | "i16"
                            | "i32"
                            | "i64"
                            | "bool"
                            | "usize"
                            | "isize"
                            | "lv_coord_t"
                            | "lv_state_t"
                            | "lv_obj_flag_t"
                            | "lv_style_selector_t"
                            | "lv_flex_flow_t"
                            | "lv_opa_t"
                    )
                }
                Type::Ptr(ptr) => ptr.elem.to_token_stream().to_string() == "lv_obj_t",
                _ => false,
            }
        }
        self.signature.variadic.is_none()
            && self.signature.inputs.iter().all(|arg| {
                let FnArg::Typed(arg) = arg else { return false };
                let Pat::Ident(name) = &*arg.pat else {
                    return false;
                };
                !name.ident.to_string().starts_with("__") && safe_type(&arg.ty)
            })
            && match &self.signature.output {
                ReturnType::Default => true,
                ReturnType::Type(_, ty) => safe_type(ty),
            }
    }
}

#[derive(Debug)]
pub struct CallCatalog {
    calls: Vec<Call>,
}

impl CallCatalog {
    pub fn parse(source: &str) -> Result<Self, GeneratorError> {
        let context =
            LvglContext::parse(source).map_err(|error| GeneratorError(error.to_string()))?;
        Self::from_signatures(context.get_signatures())
    }

    pub fn from_signatures(mut signatures: Vec<Signature>) -> Result<Self, GeneratorError> {
        signatures.sort_by_key(|sig| sig.ident.to_string().to_lowercase());
        let mut names = BTreeSet::new();
        let calls = signatures
            .into_iter()
            .enumerate()
            .map(|(id, signature)| {
                let name = get_function_identifier("", &signature.ident).to_string();
                if !names.insert(name.clone()) {
                    return Err(GeneratorError(format!("duplicate guest call {name}")));
                }
                let mut argument_count = 0;
                let mut host_arguments = Vec::new();
                for argument in &signature.inputs {
                    let FnArg::Typed(argument) = argument else {
                        return Err(GeneratorError(format!("receiver in {name}")));
                    };
                    let Pat::Ident(ident) = &*argument.pat else {
                        return Err(GeneratorError(format!("unnamed argument in {name}")));
                    };
                    let ident = ident.ident.to_string();
                    if ident.starts_with("__") {
                        host_arguments.push(ident);
                    } else {
                        argument_count += 1;
                    }
                }
                if argument_count > MAX_ARGUMENTS {
                    return Err(GeneratorError(format!(
                        "{name} exceeds seven guest arguments"
                    )));
                }
                Ok(Call {
                    id: u16::try_from(id)
                        .map_err(|_| GeneratorError("too many call IDs".into()))?,
                    name,
                    signature,
                    argument_count,
                    host_arguments,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { calls })
    }

    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    pub fn signatures(&self) -> Vec<Signature> {
        self.calls
            .iter()
            .map(|call| call.signature.clone())
            .collect()
    }
}
