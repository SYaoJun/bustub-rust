use crate::{BustubxError, BustubxResult};
use std::sync::Arc;

use crate::planner::logical_plan::{Insert, LogicalPlan};

use super::LogicalPlanner;

impl<'a> LogicalPlanner<'a> {
    pub fn plan_insert(
        &self,
        table_name: &sqlparser::ast::ObjectName,
        columns_ident: &Vec<sqlparser::ast::Ident>,
        source: &sqlparser::ast::Query,
    ) -> BustubxResult<LogicalPlan> {
        let values = self.plan_set_expr(source.body.as_ref())?;
        let table = self.bind_table_name(table_name)?;
        let table_schema = self
            .context
            .catalog
            .get_table_by_name(table.table())
            .map_or(
                Err(BustubxError::Plan(format!("table {} not found", table))),
                |info| Ok(info.schema.clone()),
            )?;

        let projected_schema = if columns_ident.is_empty() {
            table_schema.clone()
        } else {
            let columns: Vec<String> = columns_ident
                .iter()
                .map(|ident| ident.value.clone())
                .collect();
            let indices = columns
                .iter()
                .map(|name| table_schema.index_of(name.as_str()))
                .collect::<BustubxResult<Vec<usize>>>()?;
            let projected_schema = table_schema.project(&indices)?;
            projected_schema
        };

        Ok(LogicalPlan::Insert(Insert {
            table,
            table_schema,
            projected_schema,
            input: Arc::new(values),
        }))
    }
}
