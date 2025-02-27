use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::plonk::ConstraintSystem;
use halo2_proofs::plonk::Expression;
use halo2_proofs::plonk::VirtualCells;
use specs::external_host_call_table::encode::encode_host_call_entry;
use std::marker::PhantomData;

use crate::circuits::traits::ConfigureLookupTable;
use crate::curr;
use crate::fixed_curr;

use super::ExternalHostCallTableConfig;

impl<F: FieldExt> ExternalHostCallTableConfig<F> {
    pub(in crate::circuits) fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        Self {
            idx: meta.fixed_column(),
            op: meta.advice_column(),
            is_ret: meta.advice_column(),
            arg: meta.advice_column(),
            _phantom: PhantomData,
        }
    }
}

impl<F: FieldExt> ConfigureLookupTable<F> for ExternalHostCallTableConfig<F> {
    fn configure_in_table(
        &self,
        meta: &mut ConstraintSystem<F>,
        key: &'static str,
        expr: impl FnOnce(&mut VirtualCells<'_, F>) -> Expression<F>,
    ) {
        meta.lookup_any(key, |meta| {
            vec![(
                expr(meta),
                encode_host_call_entry(
                    fixed_curr!(meta, self.idx),
                    curr!(meta, self.op),
                    curr!(meta, self.is_ret),
                    curr!(meta, self.arg),
                ),
            )]
        });
    }
}
