mod callback_interface;
mod compounds;
mod custom;
mod enum_;
mod filters;
mod miscellany;
mod object;
mod primitives;
mod record;

use std::{
    borrow::Borrow,
    cell::RefCell,
    cmp::Ordering,
    collections::{BTreeSet, HashMap},
};

use anyhow::{Context, Result};
use askama::Template;
use serde::{Deserialize, Serialize};
use uniffi_bindgen::{interface::{AsType, Type}, BindingsConfig, ComponentInterface};

#[derive(Clone, Deserialize, Serialize)]
struct CustomTypesConfig {
    imports: Option<Vec<String>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Config {
    cdylib_name: Option<String>,
    #[serde(default)]
    custom_types: HashMap<String, CustomTypesConfig>,
}

impl BindingsConfig for Config {
    fn update_from_ci(&mut self, ci: &ComponentInterface) {
        self.cdylib_name.get_or_insert_with(|| format!("uniffi_{}", ci.namespace()));
    }

    fn update_from_cdylib_name(&mut self, cdylib_name: &str) {
        self.cdylib_name.get_or_insert_with(|| cdylib_name.to_string());
    }

    fn update_from_dependency_configs(&mut self, _config_map: HashMap<&str, &Self>) {}
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "types.cpp")]
struct TypeRenderer<'a> {
    ci: &'a ComponentInterface,
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "scaffolding.hpp")]
struct ScaffoldingHeader<'a> {
    ci: &'a ComponentInterface,
}

impl<'a> ScaffoldingHeader<'a> {
    fn new(ci: &'a ComponentInterface) -> Self {
        Self { ci }
    }
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "wrapper.hpp")]
struct CppWrapperHeader<'a> {
    ci: &'a ComponentInterface,
    config: &'a Config,
    includes: RefCell<BTreeSet<String>>,
}

impl<'a> CppWrapperHeader<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a Config) -> Self {
        Self {
            ci,
            config,
            includes: RefCell::new(BTreeSet::new()),
        }
    }

    pub(crate) fn sorted_types(
        &self,
        types: impl Iterator<Item = &'a Type>,
    ) -> impl Iterator<Item = &'a Type> {
        let (mut recs, rest): (Vec<&'a Type>, Vec<&'a Type>) = types
            .partition(|t| matches!(t, Type::Record { .. }));
        let (cbs, rest): (Vec<&'a Type>, Vec<&'a Type>) = rest
            .iter()
            .partition(|t| matches!(t, Type::CallbackInterface { .. }));
        let (csts, rest): (Vec<&'a Type>, Vec<&'a Type>) = rest
            .iter()
            .partition(|t| matches!(t, Type::Custom { .. }));

        recs.sort_by(|a, _| {
            match a {
                Type::Record { name, .. } => {
                    match self.ci.get_record_definition(name) {
                        Some(rec) => {
                            if rec.fields().iter().any(|field| matches!(field.as_type(), Type::Record{ .. })) {
                                return Ordering::Less;
                            }
                        },
                        None => unreachable!()
                    }
                },
                _ => unreachable!()
            }

            Ordering::Equal
        });

        csts.into_iter().chain(recs).chain(cbs).chain(rest)
    }

    pub(crate) fn add_include(&self, include: &str) -> &str {
        self.includes.borrow_mut().insert(include.to_string());

        ""
    }

    pub(crate) fn includes(&self) -> Vec<String> {
        self.includes.borrow().iter().cloned().collect()
    }
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "wrapper.cpp")]
struct CppWrapper<'a> {
    ci: &'a ComponentInterface,
    config: &'a Config,
    type_helper_code: String,
}

impl<'a> CppWrapper<'a> {
    pub(crate) fn new(ci: &'a ComponentInterface, config: &'a Config) -> Self {
        Self {
            ci,
            config,
            type_helper_code: TypeRenderer { ci }.render().unwrap(),
        }
    }
}

pub(crate) struct Bindings {
    pub(crate) scaffolding_header: String,
    pub(crate) header: String,
    pub(crate) source: String,
}

pub(crate) fn generate_cpp_bindings(
    ci: &ComponentInterface,
    config: &Config,
) -> Result<Bindings> {
    let scaffolding_header = ScaffoldingHeader::new(ci)
        .render()
        .context("generating scaffolding header failed")?;
    let header = CppWrapperHeader::new(ci, config)
        .render()
        .context("generating C++ bindings header failed")?;
    let source = CppWrapper::new(ci, config)
        .render()
        .context("generating C++ bindings failed")?;

    Ok(Bindings { scaffolding_header, header, source })
}
