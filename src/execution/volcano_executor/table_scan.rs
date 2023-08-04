use crate::execution::execution_plan::ExecutionPlan;
use crate::{
    execution::ExecutionContext, optimizer::operator::PhysicalOperator, storage::tuple::Tuple,
};
use std::sync::Arc;

use super::VolcanoExecutor;

#[derive(Debug)]
pub struct VolcanoTableScanExecutor;
impl VolcanoExecutor for VolcanoTableScanExecutor {
    fn init(&mut self) {
        todo!()
    }
    fn next(
        &self,
        context: &mut ExecutionContext,
        op: Arc<PhysicalOperator>,
        children: Vec<Arc<ExecutionPlan>>,
    ) -> Option<Tuple> {
        todo!()
    }
}