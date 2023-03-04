// RGB Core Library: consensus layer for RGB smart contracts.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2019-2023 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2023 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2019-2023 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{btree_map, BTreeMap};
use std::io;

use aluvm::data::encoding::{Decode, Encode};
use aluvm::library::{Lib, LibId, LibSite};
use aluvm::Program;
use amplify::confinement::{Confined, SmallBlob, SmallOrdMap, TinyOrdMap};
use strict_encoding::{
    DecodeError, ReadStruct, StrictDecode, StrictEncode, StrictProduct, StrictStruct, StrictType,
    TypedRead, TypedWrite, WriteStruct,
};

use crate::vm::RgbIsa;
use crate::{ExtensionType, GlobalStateType, OwnedStateType, TransitionType, LIB_NAME_RGB};

/// Maximum total number of libraries which may be used by a single program;
/// i.e. maximal number of nodes in a library dependency tree.
pub const LIBS_MAX_TOTAL: usize = 1024;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = LIB_NAME_RGB, tags = order)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
pub enum EntryPoint {
    #[strict_type(dumb)]
    ValidateGenesis,
    ValidateTransition(TransitionType),
    ValidateExtension(ExtensionType),
    ValidateGlobalState(GlobalStateType),
    ValidateOwnedState(OwnedStateType),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
//#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
//#[strict_type(lib = LIB_NAME_RGB)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
pub struct AluScript {
    /// Libraries known to the runtime, identified by their hashes.
    pub libs: Confined<BTreeMap<LibId, Lib>, 0, LIBS_MAX_TOTAL>,

    /// Set of entry points.
    pub entry_points: SmallOrdMap<EntryPoint, LibSite>,
}

// TODO: Remove this once aluvm::Lib will support strict encoding
impl StrictType for AluScript {
    const STRICT_LIB_NAME: &'static str = LIB_NAME_RGB;
}
impl StrictProduct for AluScript {}
impl StrictStruct for AluScript {
    const ALL_FIELDS: &'static [&'static str] = &["libs", "entryPoints"];
}
impl StrictEncode for AluScript {
    fn strict_encode<W: TypedWrite>(&self, writer: W) -> io::Result<W> {
        let libs = self.libs.iter().map(|(id, lib)| {
            let lib = SmallBlob::try_from(lib.serialize()).expect(
                "the RGB Core library must not be used to create AluVM library size exceeding 2^16",
            );
            (*id, lib)
        });

        writer.write_struct::<Self>(|w| {
            Ok(w.write_field(
                fname!("libs"),
                &TinyOrdMap::try_from_iter(libs).expect(
                    "the RGB Core library must not be used to create AluVM scripts with more than \
                     255 libraries",
                ),
            )?
            .write_field(fname!("entryPoints"), &self.entry_points)?
            .complete())
        })
    }
}
impl StrictDecode for AluScript {
    fn strict_decode(reader: &mut impl TypedRead) -> Result<Self, DecodeError> {
        reader.read_struct(|r| {
            let libs = r
                .read_field::<TinyOrdMap<LibId, SmallBlob>>(fname!("libs"))?
                .into_iter()
                .map(|(id, lib)| {
                    let lib = Lib::deserialize(lib)
                        .map_err(|err| DecodeError::DataIntegrityError(err.to_string()))?;
                    Ok((id, lib))
                })
                .collect::<Result<BTreeMap<_, _>, DecodeError>>()?;

            let entry_points = r.read_field(fname!("entryPoints"))?;
            Ok(AluScript {
                libs: Confined::try_from(libs).expect("strict decoder guarantees"),
                entry_points,
            })
        })
    }
}

impl Program for AluScript {
    type Isa = RgbIsa;
    type Iter<'a> = btree_map::Values<'a, LibId, Lib> where Self: 'a;

    fn lib_count(&self) -> u16 { self.libs.len() as u16 }

    fn libs(&self) -> Self::Iter<'_> { self.libs.values() }

    fn lib(&self, id: LibId) -> Option<&Lib> { self.libs.get(&id) }

    fn entrypoint(&self) -> LibSite { panic!("AluScript doesn't have a single entry point") }
}