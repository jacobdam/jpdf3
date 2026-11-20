//! Whole-model consistency checks (specs/tooling/arlington.md, "Checks").

use std::collections::BTreeSet;

use crate::Error;
use crate::model::{Model, Since};

/// Returns every problem at once, so one run shows them all.
pub fn check(model: &Model) -> Result<(), Error> {
    let mut problems = Vec::new();
    for (name, object) in &model.objects {
        let mut seen = BTreeSet::new();
        for key in &object.keys {
            let at = format!("{name}/{}", key.key);
            if !seen.insert(&key.key) {
                problems.push(format!("{at}: duplicate key"));
            }
            if key.indirect.len() != 1 && key.indirect.len() != key.types.len() {
                problems.push(format!(
                    "{at}: {} IndirectReference values for {} types",
                    key.indirect.len(),
                    key.types.len()
                ));
            }
            if let (Since::Version(since), Some(dep)) = (&key.since, key.deprecated)
                && dep < *since
            {
                problems.push(format!(
                    "{at}: deprecated in {dep}, before it appeared in {since}"
                ));
            }
            let mut types = BTreeSet::new();
            for alt in &key.types {
                let ty = alt.ty.value;
                if !types.insert(ty) {
                    problems.push(format!("{at}: type {} listed twice", ty.arlington_name()));
                }
                if ty.needs_link() && alt.links.is_empty() {
                    problems.push(format!("{at}: {} has no link", ty.arlington_name()));
                }
                if !ty.may_link() && !alt.links.is_empty() {
                    problems.push(format!(
                        "{at}: {} cannot link to objects",
                        ty.arlington_name()
                    ));
                }
                for link in &alt.links {
                    if !model.objects.contains_key(&link.value) {
                        problems.push(format!("{at}: link to missing object {}", link.value));
                    }
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::Check(problems))
    }
}
