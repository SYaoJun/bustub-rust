use crate::catalog::{Column, Schema, EMPTY_SCHEMA_REF};
use crate::expression::{Alias, ColumnExpr, Expr, ExprTrait};
use crate::planner::logical_plan::JoinType;
use crate::planner::logical_plan::{
    build_join_schema, project_schema, EmptyRelation, Filter, Join, LogicalPlan, Project,
    TableScan, Values,
};
use crate::planner::LogicalPlanner;
use crate::{BustubxError, BustubxResult};
use std::sync::Arc;

impl LogicalPlanner<'_> {
    pub fn plan_set_expr(&self, set_expr: &sqlparser::ast::SetExpr) -> BustubxResult<LogicalPlan> {
        match set_expr {
            sqlparser::ast::SetExpr::Select(select) => self.plan_select(select),
            sqlparser::ast::SetExpr::Values(values) => self.plan_values(values),
            _ => Err(BustubxError::Plan(format!(
                "Failed to plan set expr: {}",
                set_expr
            ))),
        }
    }

    pub fn plan_select(&self, select: &sqlparser::ast::Select) -> BustubxResult<LogicalPlan> {
        let table_scan = self.plan_from_tables(&select.from)?;
        let selection = self.plan_selection(table_scan, &select.selection)?;
        self.plan_project(selection, &select.projection)
    }

    pub fn plan_project(
        &self,
        input: LogicalPlan,
        project: &Vec<sqlparser::ast::SelectItem>,
    ) -> BustubxResult<LogicalPlan> {
        let mut exprs = vec![];
        for select_item in project {
            match select_item {
                sqlparser::ast::SelectItem::UnnamedExpr(expr) => exprs.push(self.bind_expr(expr)?),
                sqlparser::ast::SelectItem::ExprWithAlias { expr, alias } => {
                    exprs.push(Expr::Alias(Alias {
                        name: alias.value.clone(),
                        expr: Box::new(self.bind_expr(expr)?),
                    }))
                }
                sqlparser::ast::SelectItem::Wildcard(_) => {
                    let all_columns = input
                        .schema()
                        .columns
                        .iter()
                        .map(|col| {
                            Expr::Column(ColumnExpr {
                                relation: col.relation.clone(),
                                name: col.name.clone(),
                            })
                        })
                        .collect::<Vec<Expr>>();
                    exprs.extend(all_columns);
                }
                _ => {
                    return Err(BustubxError::Plan(format!(
                        "sqlparser select item {} not supported",
                        select_item
                    )));
                }
            }
        }
        let schema = Arc::new(project_schema(&input, &exprs)?);
        Ok(LogicalPlan::Project(Project {
            exprs,
            input: Arc::new(input),
            schema,
        }))
    }

    pub fn plan_selection(
        &self,
        input: LogicalPlan,
        selection: &Option<sqlparser::ast::Expr>,
    ) -> BustubxResult<LogicalPlan> {
        match selection {
            None => Ok(input),
            Some(predicate) => {
                let predicate = self.bind_expr(predicate)?;
                Ok(LogicalPlan::Filter(Filter {
                    input: Arc::new(input),
                    predicate,
                }))
            }
        }
    }

    pub fn plan_from_tables(
        &self,
        from: &Vec<sqlparser::ast::TableWithJoins>,
    ) -> BustubxResult<LogicalPlan> {
        match from.len() {
            0 => Ok(LogicalPlan::EmptyRelation(EmptyRelation {
                produce_one_row: true,
                schema: Arc::new(Schema::empty()),
            })),
            1 => self.plan_table_with_joins(&from[0]),
            _ => {
                let mut left = self.plan_table_with_joins(&from[0])?;
                for t in from.iter().skip(1) {
                    let right = self.plan_table_with_joins(t)?;
                    left = self.plan_cross_join(left, right)?;
                }
                Ok(left)
            }
        }
    }

    pub fn plan_table_with_joins(
        &self,
        t: &sqlparser::ast::TableWithJoins,
    ) -> BustubxResult<LogicalPlan> {
        let mut left = self.plan_relation(&t.relation)?;
        match t.joins.len() {
            0 => Ok(left),
            _ => {
                for join in t.joins.iter() {
                    left = self.plan_relation_join(left, join)?;
                }
                Ok(left)
            }
        }
    }

    pub fn plan_relation_join(
        &self,
        left: LogicalPlan,
        join: &sqlparser::ast::Join,
    ) -> BustubxResult<LogicalPlan> {
        let right = self.plan_relation(&join.relation)?;
        match &join.join_operator {
            sqlparser::ast::JoinOperator::Inner(constraint) => {
                self.plan_join(left, right, constraint, JoinType::Inner)
            }
            sqlparser::ast::JoinOperator::LeftOuter(constraint) => {
                self.plan_join(left, right, constraint, JoinType::Inner)
            }
            sqlparser::ast::JoinOperator::RightOuter(constraint) => {
                self.plan_join(left, right, constraint, JoinType::Inner)
            }
            sqlparser::ast::JoinOperator::FullOuter(constraint) => {
                self.plan_join(left, right, constraint, JoinType::Inner)
            }
            sqlparser::ast::JoinOperator::CrossJoin => self.plan_cross_join(left, right),
            _ => Err(BustubxError::Plan(format!(
                "sqlparser join operator {:?} not supported",
                join.join_operator
            ))),
        }
    }

    pub fn plan_join(
        &self,
        left: LogicalPlan,
        right: LogicalPlan,
        constraint: &sqlparser::ast::JoinConstraint,
        join_type: JoinType,
    ) -> BustubxResult<LogicalPlan> {
        match constraint {
            sqlparser::ast::JoinConstraint::On(expr) => {
                let expr = self.bind_expr(expr)?;
                let schema = Arc::new(build_join_schema(left.schema(), right.schema(), join_type)?);
                Ok(LogicalPlan::Join(Join {
                    left: Arc::new(left),
                    right: Arc::new(right),
                    join_type,
                    condition: Some(expr),
                    schema,
                }))
            }
            _ => Err(BustubxError::Plan(format!(
                "Only support join on constraint, {:?}",
                constraint
            ))),
        }
    }

    pub fn plan_cross_join(
        &self,
        left: LogicalPlan,
        right: LogicalPlan,
    ) -> BustubxResult<LogicalPlan> {
        let schema = Arc::new(build_join_schema(
            left.schema(),
            right.schema(),
            JoinType::Cross,
        )?);
        Ok(LogicalPlan::Join(Join {
            left: Arc::new(left),
            right: Arc::new(right),
            join_type: JoinType::Cross,
            condition: None,
            schema,
        }))
    }

    pub fn plan_relation(
        &self,
        relation: &sqlparser::ast::TableFactor,
    ) -> BustubxResult<LogicalPlan> {
        match relation {
            sqlparser::ast::TableFactor::Table { name, alias, .. } => {
                // TODO handle alias
                let table_ref = self.bind_table_name(name)?;
                // TODO get schema by full table name
                let schema = self
                    .context
                    .catalog
                    .get_table_by_name(table_ref.table())
                    .map_or(
                        Err(BustubxError::Plan(format!("table {} not found", table_ref))),
                        |info| Ok(info.schema.clone()),
                    )?;
                Ok(LogicalPlan::TableScan(TableScan {
                    table_ref,
                    table_schema: schema,
                    filters: vec![],
                    limit: None,
                }))
            }
            sqlparser::ast::TableFactor::NestedJoin {
                table_with_joins,
                alias,
            } => {
                // TODO handle alias
                self.plan_table_with_joins(table_with_joins)
            }
            sqlparser::ast::TableFactor::Derived {
                subquery, alias, ..
            } => self.plan_query(subquery),
            _ => Err(BustubxError::Plan(format!(
                "sqlparser relation {} not supported",
                relation
            ))),
        }
    }

    pub fn plan_values(&self, values: &sqlparser::ast::Values) -> BustubxResult<LogicalPlan> {
        let mut result = vec![];
        for row in values.rows.iter() {
            let mut record = vec![];
            for item in row {
                record.push(self.bind_expr(item)?);
            }
            result.push(record);
        }
        if result.is_empty() {
            return Ok(LogicalPlan::Values(Values {
                schema: Arc::new(Schema::empty()),
                values: vec![],
            }));
        }

        // parse schema
        let first_row = &result[0];
        let mut columns = vec![];
        for (idx, item) in first_row.iter().enumerate() {
            columns.push(Column::new(
                idx.to_string(),
                item.data_type(&EMPTY_SCHEMA_REF)?,
                item.nullable(&EMPTY_SCHEMA_REF)?,
            ))
        }

        Ok(LogicalPlan::Values(Values {
            schema: Arc::new(Schema::new(columns)),
            values: result,
        }))
    }
}
