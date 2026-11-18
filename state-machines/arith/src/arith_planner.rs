//! The `ArithPlanner` module defines a planner for generating execution plans specific to
//! arithmetic operations.
//!
//! The `Arith` airs are one air at several heights (`Arith`, `ArithSmall`): they prove the same
//! operations and commit the same columns. The planner sizes them together under the shared
//! criterion (fewest instances first, least memory to break a tie; see [`select_sizes`]) and then
//! cuts the operations at each granted instance's own capacity, roomiest first, so the tail lands
//! in the smallest air that holds it.

use crate::ArithCounterInputGen;
use zisk_common::{
    plan_ladder_with_frops, select_sizes, AirChoice, BusDeviceMetrics, CheckPoint, ChunkId,
    InstFropsCount, InstanceInfo, InstanceType, Metrics, Plan, Planner,
};

/// The `ArithPlanner` struct organizes execution plans for the arithmetic airs.
#[derive(Default)]
pub struct ArithPlanner {
    /// The heights of the arithmetic air, each with the memory one instance of it costs.
    airs: Vec<(InstanceInfo, usize)>,
}

impl ArithPlanner {
    /// Creates a new `ArithPlanner` with no airs.
    pub fn new() -> Self {
        Self { airs: Vec::new() }
    }

    /// Adds one height of the arithmetic air and the memory (`*_INSTANCE_COST`) one instance of
    /// it costs.
    pub fn add_air(mut self, instance_info: InstanceInfo, cost: usize) -> Self {
        self.airs.push((instance_info, cost));
        self
    }
}

impl Planner for ArithPlanner {
    /// Generates execution plans for the arithmetic instances.
    ///
    /// # Panics
    /// Panics if any counter cannot be downcasted to an `ArithCounterInputGen`, or if the airs do
    /// not share one operation type.
    fn plan(&self, counters: Vec<(ChunkId, Box<dyn BusDeviceMetrics>)>) -> Vec<Plan> {
        let Some((first, _)) = self.airs.first() else { return Vec::new() };
        let op_type = first.op_type;
        assert!(
            self.airs.iter().all(|(info, _)| info.op_type == op_type),
            "ArithPlanner: the arithmetic airs are one size ladder, so they must share their op_type"
        );

        let count: Vec<InstFropsCount> = counters
            .iter()
            .map(|(chunk_id, counter)| {
                let reg_counter =
                    Metrics::as_any(&**counter).downcast_ref::<ArithCounterInputGen>().unwrap();
                InstFropsCount::new(
                    *chunk_id,
                    reg_counter.inst_count(op_type).unwrap(),
                    reg_counter.frops_count(op_type).unwrap(),
                )
            })
            .collect();

        let ladder: Vec<AirChoice> = self
            .airs
            .iter()
            .map(|(info, cost)| AirChoice::new(info.airgroup_id, info.air_id, info.num_ops, *cost))
            .collect();

        let total: u64 = count.iter().map(|c| c.inst_count).sum();
        let mut granted = select_sizes(total, &ladder);
        // Frequent operations take no rows, but some instance has to collect them.
        if total == 0 && count.iter().any(|c| c.frops_count > 0) {
            let smallest = (0..ladder.len()).min_by_key(|&i| ladder[i].memory).unwrap();
            granted[smallest] = 1;
        }
        tracing::debug!(
            "··· Arith ladder: {total} ops in {granted:?} (instances of each air, capacities {:?})",
            ladder.iter().map(|air| air.rows).collect::<Vec<_>>(),
        );

        // Roomiest first, so the tail is what lands in the small one the sizing demoted.
        let mut order: Vec<usize> = (0..ladder.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(ladder[i].rows));
        let mut instance_air: Vec<usize> = Vec::new();
        let mut capacities: Vec<u64> = Vec::new();
        for air in order {
            for _ in 0..granted[air] {
                instance_air.push(air);
                capacities.push(ladder[air].rows);
            }
        }

        plan_ladder_with_frops(&count, &capacities)
            .into_iter()
            .map(|(instance, check_point, collect_info): (usize, CheckPoint, _)| {
                let air = ladder[instance_air[instance]];
                Plan::new(
                    air.airgroup_id,
                    air.air_id,
                    None,
                    InstanceType::Instance,
                    check_point,
                    Some(Box::new(collect_info)),
                )
            })
            .collect()
    }
}
