use crate::runtime::host::host_env::HostEnv;
use crate::runtime::wasmi_interpreter::WasmRuntimeIO;
use crate::runtime::ExecutionResult;
use anyhow::Result;
use wasmi::RuntimeValue;

use super::compile_then_execute_wasm;

/*
   unsigned long long wasm_input(int);

   unsigned long long fib(unsigned long long n)
   {
       if (n <= 1)
           return n;
       return fib(n - 1) + fib(n - 2);
   }

   unsigned long long test() {
       unsigned long long input = wasm_input(1);
       return fib(input);
   }
*/
fn build_test() -> Result<(ExecutionResult<RuntimeValue>, i32)> {
    let textual_repr = r#"
    (module
        (type (;0;) (func (param i32) (result i32)))
        (type (;1;) (func (result i32)))
        (func (;0;) (type 0) (param i32) (result i32)
          (local i32)
          local.get 0
          i32.const 2
          i32.ge_u
          if  ;; label = @1
            loop  ;; label = @2
              local.get 0
              i32.const 1
              i32.sub
              call 0
              local.get 1
              i32.add
              local.set 1
              local.get 0
              i32.const 2
              i32.sub
              local.tee 0
              i32.const 1
              i32.gt_u
              br_if 0 (;@2;)
            end
          end
          local.get 0
          local.get 1
          i32.add)
        (func (;1;) (type 1) (result i32)
          i32.const 10
          call 0)
        (memory (;0;) 2 2)
        (export "memory" (memory 0))
        (export "zkmain" (func 1)))
    "#;

    let wasm = wabt::wat2wasm(&textual_repr).expect("failed to parse wat");

    let mut env = HostEnv::new();
    env.finalize();

    let trace = compile_then_execute_wasm(env, WasmRuntimeIO::empty(), wasm, "zkmain")?;

    Ok((trace, 55))
}

mod tests {
    use super::*;
    use crate::circuits::ZkWasmCircuitBuilder;
    use crate::test::test_circuit_mock;
    use halo2_proofs::pairing::bn256::Fr as Fp;

    #[test]
    fn test_fibonacci_mock() {
        let (trace, expected_value) = build_test().unwrap();

        assert_eq!(trace.result.unwrap(), RuntimeValue::I32(expected_value));

        test_circuit_mock::<Fp>(trace).unwrap();
    }

    #[test]
    fn test_fibonacci_full() {
        let (execution_result, expected_value) = build_test().unwrap();

        assert_eq!(
            execution_result.result.unwrap(),
            RuntimeValue::I32(expected_value)
        );

        let builder = ZkWasmCircuitBuilder {
            tables: execution_result.tables,
            public_inputs_and_outputs: execution_result.public_inputs_and_outputs,
        };

        builder.bench()
    }
}
