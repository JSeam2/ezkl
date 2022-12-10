use crate::tensor::TensorType;
use crate::tensor::{Tensor, ValTensor};
use anyhow::Result;
use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{Circuit, ConstraintSystem, Error},
};
use std::marker::PhantomData;
pub mod utilities;
pub use utilities::*;
pub mod model;
pub mod node;
pub mod vars;
use log::{info, trace};
pub use model::*;
pub use node::*;
use std::cmp::max;
pub use vars::*;

#[derive(Clone, Debug)]
pub struct ModelCircuit<F: FieldExt> {
    pub inputs: Vec<Tensor<i32>>,
    pub _marker: PhantomData<F>,
}

impl<F: FieldExt + TensorType> Circuit<F> for ModelCircuit<F> {
    type Config = ModelConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure(cs: &mut ConstraintSystem<F>) -> Self::Config {
        let model = Model::from_arg();
        let num_advice: usize;
        let mut num_fixed = 0;
        let row_cap = model.max_node_size();

        // TODO: extract max number of params in a given fused layer
        if model.visibility.params.is_public() {
            num_fixed += model.max_node_params();
            // this is the maximum of variables in non-fused layer, and the maximum of variables (non-params) in fused layers
            num_advice = max(model.max_node_vars_non_fused(), model.max_node_vars_fused());
        } else {
            // this is the maximum of variables in non-fused layer, and the maximum of variables (non-params) in fused layers
            //  + the max number of params in a fused layer
            num_advice = max(
                model.max_node_vars_non_fused(),
                model.max_node_params() + model.max_node_vars_fused(),
            );
        }
        // for now the number of instances corresponds to the number of graph / model outputs
        let mut num_instances = 0;
        let mut instance_shapes = vec![];
        if model.visibility.input.is_public() {
            num_instances += model.num_inputs();
            instance_shapes.extend(model.input_shapes());
        }
        if model.visibility.output.is_public() {
            num_instances += model.num_outputs();
            instance_shapes.extend(model.output_shapes());
        }
        let mut vars = ModelVars::new(
            cs,
            model.logrows as usize,
            (num_advice, row_cap),
            (num_fixed, row_cap),
            (num_instances, instance_shapes),
        );
        info!("row cap: {:?}", row_cap);
        info!(
            "number of advices used: {:?}",
            vars.advices.iter().map(|a| a.num_cols()).sum::<usize>()
        );
        info!(
            "number of fixed used: {:?}",
            vars.fixed.iter().map(|a| a.num_cols()).sum::<usize>()
        );
        info!("number of instances used: {:?}", num_instances);
        model.configure(cs, &mut vars).unwrap()
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        trace!("Setting input in synthesize");
        let inputs = self
            .inputs
            .iter()
            .map(|i| ValTensor::from(<Tensor<i32> as Into<Tensor<Value<F>>>>::into(i.clone())))
            .collect::<Vec<ValTensor<F>>>();
        trace!("Setting output in synthesize");
        config
            .model
            .layout(config.clone(), &mut layouter, &inputs, &config.vars)
            .unwrap();

        Ok(())
    }
}