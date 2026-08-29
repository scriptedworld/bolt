//! The mapping substitution resolves against, built in three layers.
//!
//! FR-4.16: bolt's own values, then the jig's `definitions` block, then the
//! definitions file named on the invocation, each winning over the one before
//! it. Every key in the result is a template variable, so a value a jig defined
//! and a location bolt exposed are written and read the same way.
//!
//! FR-4.16d makes bolt's layer the exception to that ordering. The locations
//! and path variables are reserved by FR-4.19 rather than overridable, so
//! nothing above them can win and the precedence rule only ever settles a key
//! two files both set.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::Error;

/// The layer a resolved value came from, as FR-9.5g records it.
///
/// Bolt's own layer is `bolt` and is written by the manifest for the locations,
/// so it does not appear here: nothing in this mapping can be bolt's, because
/// FR-4.19 refuses a jig or a file that names one.
const FROM_JIG: &str = "jig";
const FROM_FILE: &str = "file";

/// The variables reserved to bolt, by FR-4.19.
///
/// The five locations FR-4.1c names and the two path variables. A jig or a file
/// naming one refuses the run: `{base_dir}` redefined would substitute
/// something other than where FR-4.1a stands the command, and the jig would say
/// one thing while the process did another.
pub const RESERVED: [&str; 7] = [
    "project_root",
    "base_dir",
    "config_dir",
    "output_dir",
    "work_dir",
    "each_path",
    "all_paths",
];

/// A definitions file is `bolt.<name>.definitions.yaml`, by FR-4.16a.
///
/// A jig is `bolt.<name>.yaml` in the same place by FR-3.9, so a definitions
/// file is adopted, linked and spoken of exactly as a jig is, and `link-jigs`
/// can distribute a shared one.
#[must_use]
pub fn file_name(name: &str) -> String {
    format!("bolt.{name}.definitions.yaml")
}

/// One resolved definition and the layer that supplied it.
#[derive(Debug, Clone)]
pub struct Definition {
    /// The literal value, by FR-4.17a.
    pub value: String,
    /// Which layer it came from, for FR-9.5g's manifest entry.
    pub from: &'static str,
}

/// The jig and file layers, merged.
///
/// Sorted, so a manifest's variable order does not depend on a hash seed and
/// two runs over one jig produce the same file.
#[derive(Debug, Default)]
pub struct Definitions(BTreeMap<String, Definition>);

impl Definitions {
    /// Build the mapping from the jig's block and an optional file.
    ///
    /// FR-4.17 is a successive replacement by key: each layer adds the keys the
    /// layers below did not have, replaces the values of those they did, and
    /// leaves every key it does not name standing. Nothing is deep-merged,
    /// appended to or combined, so a project overriding one detail writes that
    /// one line and inherits everything else the jig shipped.
    ///
    /// # Errors
    ///
    /// [`Error::ReservedDefinition`] when either layer names one of [`RESERVED`],
    /// by FR-4.19. Checked per layer rather than after merging, so the reason can
    /// say which file to edit.
    ///
    /// [`Error::DefinitionsUnreadable`] when a named file is absent, will not
    /// parse or will not validate, by FR-4.20. An absent one is a refusal and
    /// not an empty layer: treating it as absent would leave the jig's defaults
    /// standing and run a gate the caller thought they had overridden.
    pub fn build(
        jig_block: Option<&Value>,
        config_dir: &Path,
        file: Option<&str>,
    ) -> Result<Self, Error> {
        let mut merged = BTreeMap::new();

        if let Some(block) = jig_block {
            absorb(&mut merged, block, FROM_JIG, "the jig")?;
        }

        if let Some(name) = file {
            let path = config_dir.join(file_name(name));
            let loaded =
                crate::merge::read(&path, &wrench::schemas::DEFINITIONS).map_err(|source| {
                    Error::DefinitionsUnreadable {
                        path: path.clone(),
                        reason: source.to_string(),
                    }
                })?;
            absorb(&mut merged, &loaded, FROM_FILE, &file_name(name))?;
        }

        Ok(Self(merged))
    }

    /// The value for `name`, if any layer supplies one.
    ///
    /// FR-4.18b: a layer holding the empty string supplies a value. That is a
    /// different state from no layer holding the key at all, which FR-4.18
    /// refuses, and a jig wanting a flag to carry nothing says so by defining it.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(|definition| definition.value.as_str())
    }

    /// Every key and the layer that supplied it, for FR-9.5g's manifest.
    pub fn entries(&self) -> impl Iterator<Item = (&String, &Definition)> {
        self.0.iter()
    }
}

/// Merge one layer over what is already there, refusing a reserved name.
fn absorb(
    merged: &mut BTreeMap<String, Definition>,
    layer: &Value,
    from: &'static str,
    source: &str,
) -> Result<(), Error> {
    let Some(object) = layer.as_object() else {
        return Ok(());
    };

    for (name, value) in object {
        if RESERVED.contains(&name.as_str()) {
            return Err(Error::ReservedDefinition {
                name: name.clone(),
                source: source.to_owned(),
            });
        }
        merged.insert(
            name.clone(),
            Definition {
                value: scalar(value),
                from,
            },
        );
    }
    Ok(())
}

/// A scalar as the characters a command line carries.
///
/// The schema allows a string, a number or a boolean. `to_string` on a JSON
/// string would keep its quotes, which would reach the shell inside bolt's own
/// quoting and arrive as literal characters in the argument.
fn scalar(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}
