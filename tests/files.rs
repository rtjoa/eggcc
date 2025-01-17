use eggcc::util::{Run, TestProgram};
use insta::assert_snapshot;
use libtest_mimic::Trial;

fn generate_tests(glob: &str) -> Vec<Trial> {
    let mut trials = vec![];
    let mut mk_trial = |run: Run, snapshot: bool| {
        trials.push(Trial::test(run.name(), move || {
            let result = run.run();

            if let Some(interpreted) = result.result_interpreted {
                assert_eq!(result.original_interpreted, interpreted);
            } else {
                // only assert a snapshot if we are in the "small" folder
                if snapshot {
                    assert_snapshot!(run.name(), result.visualization);
                }
            }
            Ok(())
        }))
    };

    for entry in glob::glob(glob).unwrap() {
        let f = entry.unwrap();

        if f.to_str().unwrap().contains("should_fail") || f.to_str().unwrap().contains("failing") {
            continue;
        }

        let snapshot = f.to_str().unwrap().contains("small");

        for run in Run::all_configurations_for(TestProgram::File(f)) {
            mk_trial(run, snapshot);
        }
    }

    trials
}

fn main() {
    let args = libtest_mimic::Arguments::from_args();
    let tests = generate_tests("tests/**/*.bril");
    libtest_mimic::run(&args, tests).exit();
}
