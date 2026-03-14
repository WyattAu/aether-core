(module
  (func (export "cold_start_entry") (result i32)
    i32.const 42)
  
  (func (export "state_hydrate") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
  
  (memory (export "memory") 1)
)
