// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bounded, local reference harnesses for future adiabatic adapters.
//! These functions never mutate a game, submit a cloud job, or measure energy.
use serde::{Deserialize, Serialize};

/// Upper-triangular QUBO: offset + sum Q[i,j] x[i] x[j], including the diagonal.
/// Integer coefficients make provider-result verification exact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qubo {
    pub variables: u8,
    pub offset: i64,
    pub terms: Vec<(u8, u8, i64)>,
}
impl Qubo {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=16).contains(&self.variables) || self.terms.len() > 136 {
            return Err(
                "QUBO exceeds the local reference bounds (1–16 variables, 136 terms)".into(),
            );
        }
        if self.offset.unsigned_abs() > 1_000_000 {
            return Err("QUBO offset exceeds coefficient bounds".into());
        }
        let mut seen = [[false; 16]; 16];
        for &(i, j, q) in &self.terms {
            if i > j || j >= self.variables || q.unsigned_abs() > 1_000_000 {
                return Err("Invalid upper-triangular QUBO term".into());
            }
            if seen[i as usize][j as usize] {
                return Err("Duplicate QUBO term".into());
            }
            seen[i as usize][j as usize] = true;
        }
        Ok(())
    }
    fn energy_unchecked(&self, bits: u64) -> i64 {
        self.offset
            + self
                .terms
                .iter()
                .map(|&(i, j, q)| {
                    if bits & (1 << i) != 0 && bits & (1 << j) != 0 {
                        q
                    } else {
                        0
                    }
                })
                .sum::<i64>()
    }
    pub fn energy(&self, bits: u64) -> Result<i64, String> {
        self.validate()?;
        if bits >= (1u64 << self.variables) {
            return Err("Candidate contains undeclared variables".into());
        }
        Ok(self.energy_unchecked(bits))
    }
    /// Check a local or future remote candidate; this does not establish optimality.
    pub fn verify_candidate(&self, bits: u64, claimed_energy: i64) -> Result<(), String> {
        if self.energy(bits)? != claimed_energy {
            return Err("Provider candidate has an incorrect energy".into());
        }
        Ok(())
    }
    /// Exhaustive CPU baseline, with explicit work budget and cancellation.
    /// A failed job returns no partial result and never silently changes provider.
    pub fn solve_reference(
        &self,
        max_states: u64,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(u64, i64), String> {
        self.validate()?;
        let states = 1u64 << self.variables;
        if max_states < states {
            return Err("Insufficient reference work budget".into());
        }
        let mut best = (0, i64::MAX);
        for bits in 0..states {
            if cancelled() {
                return Err("Reference job cancelled".into());
            }
            let energy = self.energy_unchecked(bits);
            if energy < best.1 {
                best = (bits, energy);
            }
        }
        Ok(best)
    }
}

/// Self-inverse logical gates; gate order is reversed for uncomputation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Gate {
    Not(u8),
    Cnot { control: u8, target: u8 },
    Toffoli { a: u8, b: u8, target: u8 },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Circuit {
    pub wires: u8,
    pub gates: Vec<Gate>,
}
impl Circuit {
    fn validate(&self, input: u64) -> Result<(), String> {
        if !(1..=63).contains(&self.wires)
            || self.gates.len() > 4096
            || input >= (1u64 << self.wires)
        {
            return Err("Circuit/register exceeds the local reference bounds".into());
        }
        for gate in &self.gates {
            let valid = match *gate {
                Gate::Not(t) => t < self.wires,
                Gate::Cnot {
                    control: c,
                    target: t,
                } => c != t && c < self.wires && t < self.wires,
                Gate::Toffoli { a, b, target: t } => {
                    a != b && a != t && b != t && a < self.wires && b < self.wires && t < self.wires
                }
            };
            if !valid {
                return Err("Gate aliases a target/control or references an absent wire".into());
            }
        }
        Ok(())
    }
    pub fn execute(&self, input: u64, inverse: bool) -> Result<u64, String> {
        self.validate(input)?;
        let mut value = input;
        for index in 0..self.gates.len() {
            let gate = self.gates[if inverse {
                self.gates.len() - 1 - index
            } else {
                index
            }];
            match gate {
                Gate::Not(t) => value ^= 1 << t,
                Gate::Cnot {
                    control: c,
                    target: t,
                } if value & (1 << c) != 0 => value ^= 1 << t,
                Gate::Toffoli { a, b, target: t }
                    if value & (1 << a) != 0 && value & (1 << b) != 0 =>
                {
                    value ^= 1 << t
                }
                _ => {}
            }
        }
        Ok(value)
    }
}

#[derive(Debug, Serialize)]
pub struct HarnessReceipt {
    pub format: &'static str,
    pub backend: &'static str,
    pub physical_hardware_tested: bool,
    pub measured_joules: Option<f64>,
    pub travel_costs: [u32; 4],
    pub model: Qubo,
    pub selected_candidate: u64,
    pub verified_energy: i64,
    pub enumerated_states: u64,
    pub reversible_input_cases: usize,
    pub clean_ancilla_wires: usize,
}

/// Exercise a game-derived advisory optimisation and a reversible half-adder.
/// The four candidate costs can come from the current district travel rules.
pub fn run_local_harness(costs: [u32; 4]) -> Result<HarnessReceipt, String> {
    if costs.iter().any(|&c| c > 100_000) {
        return Err("Travel cost exceeds harness bounds".into());
    }
    let penalty = i64::from(costs.into_iter().fold(0, u32::max)) + 1;
    // Minimise cost subject to exactly one selection: cost + P(sum(x)-1)^2.
    let mut terms = Vec::with_capacity(10);
    for (i, cost) in costs.iter().enumerate() {
        terms.push((i as u8, i as u8, i64::from(*cost) - penalty));
        for j in (i + 1)..4 {
            terms.push((i as u8, j as u8, 2 * penalty));
        }
    }
    let model = Qubo {
        variables: 4,
        offset: penalty,
        terms,
    };
    let (bits, energy) = model.solve_reference(16, || false)?;
    model.verify_candidate(bits, energy)?;
    if bits.count_ones() != 1 || energy != i64::from(costs.into_iter().fold(u32::MAX, u32::min)) {
        return Err("QUBO failed the independent minimum-cost oracle".into());
    }
    let circuit = Circuit {
        wires: 4,
        gates: vec![
            Gate::Cnot {
                control: 0,
                target: 2,
            },
            Gate::Cnot {
                control: 1,
                target: 2,
            },
            Gate::Toffoli {
                a: 0,
                b: 1,
                target: 3,
            },
        ],
    };
    for input in 0..4 {
        let output = circuit.execute(input, false)?;
        let a = input & 1;
        let b = (input >> 1) & 1;
        if (output >> 2) != ((a ^ b) | ((a & b) << 1)) || circuit.execute(output, true)? != input {
            return Err("Reversible half-adder failed truth table or clean uncomputation".into());
        }
    }
    Ok(HarnessReceipt {
        format: "cytoville-research/1",
        backend: "local-classical-reference",
        physical_hardware_tested: false,
        measured_joules: None,
        travel_costs: costs,
        model,
        selected_candidate: bits,
        verified_energy: energy,
        enumerated_states: 16,
        reversible_input_cases: 4,
        clean_ancilla_wires: 2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optimisation_matches_independent_minimum_including_ties_and_zero() {
        for costs in [[0, 0, 0, 0], [12, 4, 18, 9], [100_000; 4], [99, 3, 3, 4]] {
            let receipt = run_local_harness(costs).unwrap();
            assert_eq!(
                receipt.verified_energy,
                i64::from(*costs.iter().min().unwrap())
            );
            assert!(!receipt.physical_hardware_tested);
            assert_eq!(receipt.measured_joules, None);
        }
    }
    #[test]
    fn untrusted_models_results_and_cancelled_jobs_fail_closed() {
        let receipt = run_local_harness([1, 2, 3, 4]).unwrap();
        let mut model = receipt.model;
        assert!(model.verify_candidate(16, 1).is_err());
        assert!(model.verify_candidate(1, 99).is_err());
        assert!(model.solve_reference(15, || false).is_err());
        assert!(model.solve_reference(16, || true).is_err());
        model.terms.push(model.terms[0]);
        assert!(model.validate().is_err());
        model.variables = 64;
        assert!(model.energy(0).is_err());
    }
    #[test]
    fn inverse_restores_every_register_and_rejects_aliases() {
        let circuit = Circuit {
            wires: 3,
            gates: vec![
                Gate::Not(0),
                Gate::Cnot {
                    control: 0,
                    target: 1,
                },
                Gate::Toffoli {
                    a: 0,
                    b: 1,
                    target: 2,
                },
            ],
        };
        for input in 0..8 {
            assert_eq!(
                circuit
                    .execute(circuit.execute(input, false).unwrap(), true)
                    .unwrap(),
                input
            );
        }
        let invalid = Circuit {
            wires: 2,
            gates: vec![Gate::Cnot {
                control: 1,
                target: 1,
            }],
        };
        assert!(invalid.execute(0, false).is_err());
    }
}
