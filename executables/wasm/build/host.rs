use crate::utilities::{
    context::LvglContext, enumeration, file::write_token_stream_to_file, function::split_inputs,
};
use std::path::Path;

use proc_macro2::{Literal, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{FnArg, ItemFn, ReturnType, Signature};

fn generate_conversion_for_output(r#return: &ReturnType) -> Result<Option<TokenStream>, String> {
    match r#return {
        ReturnType::Type(_, _) => {
            let conversion = quote! {
                let __current_result = TranslateInto::translate_into(__current_result, __translator)?;
            };

            Ok(Some(quote! {
                #conversion

                *__result = __current_result;
            }))
        }
        // If the return type is not specified, we don't need to convert it
        ReturnType::Default => Ok(None),
    }
}

fn generate_conversion_for_argument(index: usize, argument: &FnArg) -> Result<TokenStream, String> {
    let argument_identifier = format_ident!("__argument_{}", index);

    let identifier = match argument {
        FnArg::Typed(pattern) => &*pattern.pat,
        _ => return Err("Unsupported argument type".to_string()),
    };

    Ok(quote! {
        let #identifier = TranslateFrom::translate_from(#argument_identifier, __translator)?;
    })
}

fn generate_call_argument(argument: &FnArg) -> Result<TokenStream, String> {
    match argument {
        FnArg::Typed(pattern) => {
            let identifier = &*pattern.pat;
            Ok(quote! {
                #identifier
            })
        }
        _ => Err("Unsupported argument type".to_string()),
    }
}

fn generate_function_call(function: &Signature) -> Result<TokenStream, String> {
    // - Get the inputs
    let inputs = function.inputs.iter().collect::<Vec<_>>();

    let (left_inputs, right_inputs) = split_inputs(&inputs)?;

    // - Generate the assignation of the arguments to variables (let name = __Argument_X;)
    let assigns = right_inputs
        .iter()
        .enumerate()
        .map(|(i, argument)| generate_conversion_for_argument(i, argument))
        .collect::<Result<Vec<_>, _>>()?;

    // - Generate the order of the arguments in the function call (name, name, ...)
    let call_arguments = left_inputs
        .iter()
        .chain(right_inputs.iter())
        .map(|argument| generate_call_argument(argument))
        .collect::<Result<Vec<_>, _>>()?;

    // - Get the number of arguments
    let arguments_count = Literal::usize_unsuffixed(right_inputs.len());

    // - Get the function identifier
    let function_identifier = &function.ident;
    let variant_identifier = enumeration::get_variant_identifier(&function.ident);

    // - Generate the return conversion if needed (let __result = __current_result;)
    let r#return = generate_conversion_for_output(&function.output)?;

    // - Generate the code for the function call (let __current_result = Function_identifier(arguments);)
    let function_call = if let Some(r#return) = &r#return {
        quote! {
            let __current_result = #function_identifier(#(
                #call_arguments,
            )*);

            let __result = __translator.translate_to_host(__result, true).ok_or(Error::InvalidPointer)?;

            #r#return
        }
    } else {
        quote! {
                #function_identifier(#(
                    #call_arguments,
                )*);

        }
    };

    Ok(quote! {
        FunctionCall::#variant_identifier => {
            // Check arguments count
            if __arguments_count != #arguments_count {
                return Err(Error::InvalidArgumentsCount);
            }
            // Assign arguments
            #(
                #assigns
            )*

            // Call function
            #function_call
        }
    })
}

pub fn generate_code(
    signatures: Vec<Signature>,
    definitions: Vec<ItemFn>,
) -> Result<TokenStream, String> {
    let functions_call = signatures
        .iter()
        .map(generate_function_call)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {

        #( #definitions )*

        #[allow(unused_variables)]
        #[allow(clippy::too_many_arguments)]
        pub unsafe fn call_function(
            __translator: &mut Translator,
            __function: FunctionCall,
            __argument_0: WasmUsize,
            __argument_1: WasmUsize,
            __argument_2: WasmUsize,
            __argument_3: WasmUsize,
            __argument_4: WasmUsize,
            __argument_5: WasmUsize,
            __argument_6: WasmUsize,
            __arguments_count: u8,
            __result: WasmPointer,
        ) -> Result<()>
        {
            use xila::graphics::lvgl::*;
            use crate::host::bindings::graphics::{
                translate::{TranslateFrom, TranslateInto},
                error::{Error},
                additionnal::*,
            };

            unsafe {
                match __function {
                    #(
                        #functions_call
                    )*

                };
            }

            Ok(())
        }
    }
    .to_token_stream())
}

pub fn generate_inner(output_path: &Path, context: &LvglContext) -> Result<(), String> {
    let output_file_path = output_path.join("bindings.rs");

    let enumerations = enumeration::generate_code(context.get_signatures());

    let functions = generate_code(context.get_signatures(), context.get_definitions())?;

    let token_stream = quote! {
        use crate::host::bindings::graphics::prelude::*;

        #enumerations

        #functions
    };

    write_token_stream_to_file(output_file_path, token_stream)?;

    Ok(())
}

pub fn generate(out_path: &Path) -> Result<(), ()> {
    let input = lvgl_rust_sys::_bindgen_raw_src();
    let context = LvglContext::parse(input).expect("Error parsing LVGL ABI");

    generate_inner(out_path, &context).expect("Error generating native bindings");

    Ok(())
}
