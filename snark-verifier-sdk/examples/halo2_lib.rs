use ark_std::{end_timer, start_timer};
use halo2_base::gates::GateChip;
use halo2_base::gates::builder::{CircuitBuilderStage, BASE_CONFIG_PARAMS, GateThreadBuilder, RangeCircuitBuilder, RangeWithInstanceCircuitBuilder};
use halo2_base::halo2_proofs::halo2curves::bn256::Fr;
use halo2_base::safe_types::{RangeChip, RangeInstructions, GateInstructions};
use halo2_base::utils::fs::gen_srs;

use snark_verifier_sdk::halo2::read_snark;
use snark_verifier_sdk::SHPLONK;
use snark_verifier_sdk::{
    gen_pk,
    halo2::{aggregation::AggregationCircuit, gen_snark_shplonk},
    Snark,
};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use snark_verifier_sdk::halo2::aggregation::VerifierUniversality;

fn generate_circuit(k: u32) -> Snark {
    let mut builder = GateThreadBuilder::new(false);
    let ctx = builder.main(0);
    let gate = GateChip::<Fr>::default();
    let range = RangeChip::<Fr>::default(8);

    let x = builder.main(0).load_witness(Fr::from(14));
    range.range_check(ctx, x, 64);
    range.gate().add(ctx, x, x);

    let circuit = RangeWithInstanceCircuitBuilder::<Fr>::keygen(builder.clone(), vec![]);
    let params = gen_srs(k);

    let pk = gen_pk(&params, &circuit, None);
    let breakpoints = circuit.break_points();

    let circuit = RangeWithInstanceCircuitBuilder::<Fr>::prover(builder.clone(), vec![], breakpoints);
    let snark = gen_snark_shplonk(&params, &pk, circuit, None::<&str>);
    snark

    
}

fn gen_agg_break_points(agg_circuit: AggregationCircuit, path: &Path) -> Vec<Vec<usize>> {
    let file = File::open(path);
    let break_points = match file {
        Ok(file) => {
            let reader = BufReader::new(file);
            let break_points: Vec<Vec<usize>> = serde_json::from_reader(reader).unwrap();
            break_points
        }
        Err(_) => {
            let break_points = agg_circuit.break_points();
            let file = File::create(path).unwrap();
            let writer = BufWriter::new(file);
            serde_json::to_writer(writer, &break_points).unwrap();
            break_points
        }
    };
    break_points
}

fn main() {
    let dummy_snark = generate_circuit(13);

    let k = 14u32;
    let params = gen_srs(k);
    let lookup_bits = k as usize - 1;
    BASE_CONFIG_PARAMS.with(|config| {
        config.borrow_mut().lookup_bits = Some(lookup_bits);
        config.borrow_mut().k = k as usize;
    });
    let agg_circuit = AggregationCircuit::new::<SHPLONK>(
        CircuitBuilderStage::Keygen,
        None,
        lookup_bits,
        &params,
        vec![dummy_snark.clone()],
        VerifierUniversality::Full,
    );
    agg_circuit.config(k, Some(10));

    let start0 = start_timer!(|| "gen vk & pk");
    let pk = gen_pk(&params, &agg_circuit, Some(Path::new("./examples/agg.pk")));
    end_timer!(start0);
    let break_points = gen_agg_break_points(agg_circuit, Path::new("./examples/break_points.json"));

    let snarks = [
        "./examples/halo2_lib_snarks/range.snark",
        "./examples/halo2_lib_snarks/halo2_lib.snark",
        "./examples/halo2_lib_snarks/poseidon.snark",
    ]
    .map(|file| read_snark_from_file(file));
    // let snarks = [dummy_snark];
    for (i, snark) in snarks.into_iter().enumerate() {
        let agg_circuit = AggregationCircuit::new::<SHPLONK>(
            CircuitBuilderStage::Prover,
            Some(break_points.clone()),
            lookup_bits,
            &params,
            vec![snark],
            VerifierUniversality::Full,
        );
        let _snark = gen_snark_shplonk(&params, &pk, agg_circuit, None::<&str>);
        println!("snark {i} success");
    }

    /*
    #[cfg(feature = "loader_evm")]
    {
        // do one more time to verify
        let num_instances = agg_circuit.num_instance();
        let instances = agg_circuit.instances();
        let proof_calldata = gen_evm_proof_shplonk(&params, &pk, agg_circuit, instances.clone());

        let deployment_code = gen_evm_verifier_shplonk::<AggregationCircuit<SHPLONK>>(
            &params,
            pk.get_vk(),
            num_instances,
            Some(Path::new("./examples/standard_plonk.yul")),
        );
        evm_verify(deployment_code, instances, proof_calldata);
    }
    */
}
