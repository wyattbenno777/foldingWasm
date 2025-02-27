use crate::circuits::bit_table::encode_bit_table_binary;
use crate::circuits::cell::*;
use crate::circuits::etable::allocator::*;
use crate::circuits::etable::ConstraintBuilder;
use crate::circuits::etable::EventTableCommonConfig;
use crate::circuits::etable::EventTableOpcodeConfig;
use crate::circuits::etable::EventTableOpcodeConfigBuilder;
use crate::circuits::utils::bn_to_field;
use crate::circuits::utils::step_status::StepStatus;
use crate::circuits::utils::table_entry::EventTableEntryWithMemoryInfo;
use crate::circuits::utils::Context;
use crate::constant;
use crate::constant_from;
use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::plonk::Error;
use halo2_proofs::plonk::Expression;
use halo2_proofs::plonk::VirtualCells;
use num_bigint::BigUint;
use specs::etable::EventTableEntry;
use specs::itable::BitOp;
use specs::itable::OpcodeClass;
use specs::itable::OPCODE_ARG0_SHIFT;
use specs::itable::OPCODE_ARG1_SHIFT;
use specs::itable::OPCODE_CLASS_SHIFT;
use specs::mtable::LocationType;
use specs::mtable::VarType;
use specs::step::StepInfo;

pub struct BinBitConfig<F: FieldExt> {
    lhs: AllocatedU64Cell<F>,
    rhs: AllocatedU64Cell<F>,
    res: AllocatedU64Cell<F>,
    op_class: AllocatedCommonRangeCell<F>,

    is_i32: AllocatedBitCell<F>,

    bit_table_lookup: AllocatedBitTableLookupCell<F>,

    memory_table_lookup_stack_read_lhs: AllocatedMemoryTableLookupReadCell<F>,
    memory_table_lookup_stack_read_rhs: AllocatedMemoryTableLookupReadCell<F>,
    memory_table_lookup_stack_write: AllocatedMemoryTableLookupWriteCell<F>,
}

pub struct BinBitConfigBuilder {}

impl<F: FieldExt> EventTableOpcodeConfigBuilder<F> for BinBitConfigBuilder {
    fn configure(
        common_config: &EventTableCommonConfig<F>,
        allocator: &mut EventTableCellAllocator<F>,
        constraint_builder: &mut ConstraintBuilder<F>,
    ) -> Box<dyn EventTableOpcodeConfig<F>> {
        let is_i32 = allocator.alloc_bit_cell();
        let lhs = allocator.alloc_u64_cell();
        let rhs = allocator.alloc_u64_cell();
        let res = allocator.alloc_u64_cell();

        let op_class = allocator.alloc_common_range_cell();

        let eid = common_config.eid_cell;
        let sp = common_config.sp_cell;

        let bit_table_lookup = common_config.bit_table_lookup_cell;

        let memory_table_lookup_stack_read_rhs = allocator.alloc_memory_table_lookup_read_cell(
            "op_bin stack read",
            constraint_builder,
            eid,
            move |____| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(1),
            move |meta| is_i32.expr(meta),
            move |meta| rhs.u64_cell.expr(meta),
            move |____| constant_from!(1),
        );

        let memory_table_lookup_stack_read_lhs = allocator.alloc_memory_table_lookup_read_cell(
            "op_bin stack read",
            constraint_builder,
            eid,
            move |____| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(2),
            move |meta| is_i32.expr(meta),
            move |meta| lhs.u64_cell.expr(meta),
            move |____| constant_from!(1),
        );

        let memory_table_lookup_stack_write = allocator.alloc_memory_table_lookup_write_cell(
            "op_bin stack read",
            constraint_builder,
            eid,
            move |____| constant_from!(LocationType::Stack as u64),
            move |meta| sp.expr(meta) + constant_from!(2),
            move |meta| is_i32.expr(meta),
            move |meta| res.u64_cell.expr(meta),
            move |____| constant_from!(1),
        );

        constraint_builder.push(
            "op_bin_bit: bit table lookup",
            Box::new(move |meta| {
                vec![
                    bit_table_lookup.0.expr(meta)
                        - encode_bit_table_binary(
                            op_class.expr(meta),
                            lhs.expr(meta),
                            rhs.expr(meta),
                            res.expr(meta),
                        ),
                ]
            }),
        );

        Box::new(BinBitConfig {
            lhs,
            rhs,
            res,
            op_class,
            is_i32,
            bit_table_lookup,
            memory_table_lookup_stack_read_lhs,
            memory_table_lookup_stack_read_rhs,
            memory_table_lookup_stack_write,
        })
    }
}

impl<F: FieldExt> EventTableOpcodeConfig<F> for BinBitConfig<F> {
    fn opcode(&self, meta: &mut VirtualCells<'_, F>) -> Expression<F> {
        constant!(bn_to_field(
            &(BigUint::from(OpcodeClass::BinBit as u64) << OPCODE_CLASS_SHIFT)
        )) + self.op_class.expr(meta)
            * constant!(bn_to_field(&(BigUint::from(1u64) << OPCODE_ARG0_SHIFT)))
            + self.is_i32.expr(meta)
                * constant!(bn_to_field(&(BigUint::from(1u64) << OPCODE_ARG1_SHIFT)))
    }

    fn assign(
        &self,
        ctx: &mut Context<'_, F>,
        step: &StepStatus,
        entry: &EventTableEntryWithMemoryInfo,
    ) -> Result<(), Error> {
        let (class, vtype, left, right, value) = match entry.eentry.step_info {
            StepInfo::I32BinBitOp {
                class,
                left,
                right,
                value,
            } => {
                let vtype = VarType::I32;
                let left = left as u32 as u64;
                let right = right as u32 as u64;
                let value = value as u32 as u64;
                (class, vtype, left, right, value)
            }
            StepInfo::I64BinBitOp {
                class,
                left,
                right,
                value,
            } => {
                let vtype = VarType::I64;
                let left = left as u64;
                let right = right as u64;
                let value = value as u64;
                (class, vtype, left, right, value)
            }
            _ => unreachable!(),
        };

        self.is_i32.assign_bool(ctx, vtype == VarType::I32)?;
        self.lhs.assign(ctx, left)?;
        self.rhs.assign(ctx, right)?;
        self.res.assign(ctx, value)?;

        self.bit_table_lookup.0.assign_bn(
            ctx,
            &encode_bit_table_binary(
                BigUint::from(class as u64),
                left.into(),
                right.into(),
                value.into(),
            ),
        )?;

        match class {
            specs::itable::BitOp::And => {
                self.op_class.assign_u32(ctx, BitOp::And as u32)?;
            }
            specs::itable::BitOp::Or => {
                self.op_class.assign_u32(ctx, BitOp::Or as u32)?;
            }
            specs::itable::BitOp::Xor => {
                self.op_class.assign_u32(ctx, BitOp::Xor as u32)?;
            }
        };

        self.memory_table_lookup_stack_read_rhs.assign(
            ctx,
            entry.memory_rw_entires[0].start_eid,
            step.current.eid,
            entry.memory_rw_entires[0].end_eid,
            step.current.sp + 1,
            LocationType::Stack,
            vtype == VarType::I32,
            right,
        )?;

        self.memory_table_lookup_stack_read_lhs.assign(
            ctx,
            entry.memory_rw_entires[1].start_eid,
            step.current.eid,
            entry.memory_rw_entires[1].end_eid,
            step.current.sp + 2,
            LocationType::Stack,
            vtype == VarType::I32,
            left,
        )?;

        self.memory_table_lookup_stack_write.assign(
            ctx,
            step.current.eid,
            entry.memory_rw_entires[2].end_eid,
            step.current.sp + 2,
            LocationType::Stack,
            vtype == VarType::I32,
            value,
        )?;

        Ok(())
    }

    fn mops(&self, _meta: &mut VirtualCells<'_, F>) -> Option<Expression<F>> {
        Some(constant_from!(1))
    }

    fn memory_writing_ops(&self, _: &EventTableEntry) -> u32 {
        1
    }

    fn sp_diff(&self, _meta: &mut VirtualCells<'_, F>) -> Option<Expression<F>> {
        Some(constant!(F::one()))
    }
}
