// Descriptor wallet library extending bitcoin & miniscript functionality
// by LNP/BP Association (https://lnp-bp.org)
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the Apache-2.0 License
// along with this software.
// If not, see <https://opensource.org/licenses/Apache-2.0>.

use std::collections::BTreeMap;
use std::convert::TryInto;

use bitcoin::util::bip32::{ExtendedPubKey, KeySource};
use bitcoin::Transaction;
#[cfg(feature = "serde")]
use serde_with::{hex::Hex, As, Same};

use crate::v0::PsbtV0;
use crate::{raw, Input, Output, PsbtVersion, TxError};

// TODO: Do manual serde implementation to check the deserialized values
#[derive(Clone, Eq, PartialEq, Debug, Default)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct Psbt {
    /// The version number of this PSBT. If omitted, the version number is 0.
    pub psbt_version: PsbtVersion,

    /// A global map from extended public keys to the used key fingerprint and
    /// derivation path as defined by BIP 32
    pub xpub: BTreeMap<ExtendedPubKey, KeySource>,

    /// Transaction version.
    pub tx_version: u32,

    // TODO: Do optional
    /// Fallback locktime (used if none of the inputs specifies their locktime).
    pub fallback_locktime: u32,

    /// The corresponding key-value map for each input.
    pub inputs: Vec<Input>,

    /// The corresponding key-value map for each output.
    pub outputs: Vec<Output>,

    /// Global proprietary key-value pairs.
    #[cfg_attr(feature = "serde", serde(with = "As::<BTreeMap<Same, Hex>>"))]
    pub proprietary: BTreeMap<raw::ProprietaryKey, Vec<u8>>,

    /// Unknown global key-value pairs.
    #[cfg_attr(feature = "serde", serde(with = "As::<BTreeMap<Same, Hex>>"))]
    pub unknown: BTreeMap<raw::Key, Vec<u8>>,
}

impl Psbt {
    /// Checks that unsigned transaction does not have scriptSig's or witness
    /// data
    pub fn with(tx: Transaction, psbt_version: PsbtVersion) -> Result<Self, TxError> {
        let inputs = tx
            .input
            .into_iter()
            .enumerate()
            .map(|(index, txin)| Input::new(index, txin).map_err(TxError::from))
            .collect::<Result<_, TxError>>()?;
        let outputs = tx
            .output
            .into_iter()
            .enumerate()
            .map(|(index, txout)| Output::new(index, txout))
            .collect();

        let i32_version = tx.version;
        let tx_version = i32_version
            .try_into()
            .map_err(|_| TxError::InvalidTxVersion(i32_version))?;

        Ok(Psbt {
            psbt_version,
            xpub: Default::default(),
            tx_version,
            fallback_locktime: tx.lock_time,
            inputs,
            outputs,
            proprietary: Default::default(),
            unknown: Default::default(),
        })
    }
}

impl From<PsbtV0> for Psbt {
    fn from(v0: PsbtV0) -> Self {
        let tx = v0.unsigned_tx;

        let inputs = v0
            .inputs
            .into_iter()
            .zip(tx.input)
            .enumerate()
            .map(|(index, (input, txin))| Input::with(index, input, txin))
            .collect();

        let outputs = v0
            .outputs
            .into_iter()
            .zip(tx.output)
            .enumerate()
            .map(|(index, (output, txout))| Output::with(index, output, txout))
            .collect();

        let tx_version = u32::from_be_bytes(tx.version.to_be_bytes());

        Psbt {
            // We need to serialize back in the same version we deserialzied from
            psbt_version: PsbtVersion::V0,
            xpub: v0.xpub,
            tx_version,
            fallback_locktime: tx.lock_time,
            inputs,
            outputs,
            proprietary: v0.proprietary,
            unknown: v0.unknown,
        }
    }
}

impl From<Psbt> for PsbtV0 {
    fn from(psbt: Psbt) -> Self {
        let version = i32::from_be_bytes(psbt.tx_version.to_be_bytes());

        let lock_time = psbt
            .inputs
            .iter()
            .filter_map(Input::locktime)
            .max()
            .unwrap_or(psbt.fallback_locktime);

        let (v0_inputs, tx_inputs) = psbt.inputs.into_iter().map(Input::split).unzip();
        let (v0_outputs, tx_outputs) = psbt.outputs.into_iter().map(Output::split).unzip();

        let unsigned_tx = Transaction {
            version,
            lock_time,
            input: tx_inputs,
            output: tx_outputs,
        };

        PsbtV0 {
            unsigned_tx,
            version: PsbtVersion::V0 as u32,
            xpub: psbt.xpub,
            proprietary: psbt.proprietary,
            unknown: psbt.unknown,
            inputs: v0_inputs,
            outputs: v0_outputs,
        }
    }
}
