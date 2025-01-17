use bril_rs::{ConstOps, Literal, Type, ValueOps};

use crate::{
    cfg::{program_to_cfg, Identifier},
    rvsdg::{cfg_to_rvsdg, new_rvsdg_egraph, EgglogFunctionResult, Expr, Id, Operand, RvsdgBody},
    util::parse_from_string,
};

use super::RvsdgFunction;

/// Utility struct for building an RVSDG.
#[derive(Default)]
struct RvsdgTest {
    nodes: Vec<RvsdgBody>,
}

impl RvsdgTest {
    /// "pure" functions are ones whose state edges 'pass through'.
    fn into_pure_function(self, n_args: usize, output: Operand) -> RvsdgFunction {
        self.into_function(n_args, Some(output), Operand::Arg(n_args))
    }

    fn into_function(
        self,
        n_args: usize,
        result: Option<Operand>,
        state: Operand,
    ) -> RvsdgFunction {
        RvsdgFunction {
            n_args,
            nodes: self.nodes,
            result,
            state,
        }
    }

    fn lit_int(&mut self, i: i64) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Const(
            ConstOps::Const,
            Literal::Int(i),
            Type::Int,
        )))
    }

    fn lit_bool(&mut self, b: bool) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Const(
            ConstOps::Const,
            Literal::Bool(b),
            Type::Bool,
        )))
    }

    fn void_function(&mut self, func: impl Into<Identifier>, args: &[Operand]) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Call(
            func.into(),
            args.to_vec(),
            1,
            None,
        )))
    }

    fn lt(&mut self, l: Operand, r: Operand) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Op(
            ValueOps::Lt,
            vec![l, r],
            Type::Bool,
        )))
    }

    fn add(&mut self, l: Operand, r: Operand, ty: Type) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Op(ValueOps::Add, vec![l, r], ty)))
    }

    fn mul(&mut self, l: Operand, r: Operand, ty: Type) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Op(ValueOps::Mul, vec![l, r], ty)))
    }

    fn print(&mut self, x: Operand, state: Operand) -> Operand {
        self.make_node(RvsdgBody::BasicOp(Expr::Print(vec![x, state])))
    }

    fn gamma(&mut self, pred: Operand, inputs: &[Operand], outputs: &[&[Operand]]) -> Id {
        let res = self.nodes.len();
        self.nodes.push(RvsdgBody::Gamma {
            pred,
            inputs: inputs.to_vec(),
            outputs: outputs.iter().map(|outs| outs.to_vec()).collect(),
        });
        res
    }

    fn theta(&mut self, pred: Operand, inputs: &[Operand], outputs: &[Operand]) -> Id {
        let res = self.nodes.len();
        self.nodes.push(RvsdgBody::Theta {
            pred,
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
        });
        res
    }

    fn make_node(&mut self, body: RvsdgBody) -> Operand {
        let res = Operand::Project(0, self.nodes.len());
        self.nodes.push(body);
        res
    }
}

#[test]
fn rvsdg_expr() {
    const PROGRAM: &str = r#"
    @sub() : int {
        v0: int = const 1;
        v1: int = const 2;
        v2: int = add v0 v1;
        ret v2;
    }
    "#;
    let prog = parse_from_string(PROGRAM);
    let cfg = program_to_cfg(&prog);
    let rvsdg = cfg_to_rvsdg(&cfg).unwrap();

    let mut expected = RvsdgTest::default();
    let one = expected.lit_int(1);
    let two = expected.lit_int(2);
    let res = expected.add(one, two, Type::Int);
    assert!(deep_equal(
        &expected.into_pure_function(0, res),
        &rvsdg.functions[0]
    ));
}

#[test]
fn rvsdg_print() {
    const PROGRAM: &str = r#"
    @sub() {
        v0: int = const 1;
        v1: int = const 2;
        v2: int = add v0 v1;
        print v2;
        print v1;
    }
    "#;
    let prog = parse_from_string(PROGRAM);
    let cfg = program_to_cfg(&prog);
    let rvsdg = cfg_to_rvsdg(&cfg).unwrap();

    let mut expected = RvsdgTest::default();
    let v0 = expected.lit_int(1);
    let v1 = expected.lit_int(2);
    let v2 = expected.add(v0, v1, Type::Int);
    let res1 = expected.print(v2, Operand::Arg(0));
    let res2 = expected.print(v1, res1);
    assert!(deep_equal(
        &expected.into_function(0, None, res2),
        &rvsdg.functions[0]
    ));
}

#[test]
fn rvsdg_state_gamma() {
    const PROGRAM: &str = r#"
    @sub() {
        x: int = const 1;
        c: bool = const true;
        br c .B .C;
    .B:
        call @some_func;
        jmp .End;
    .C:
        call @other_func;
        jmp .End;
    .End: 
    }

    @other_func() {
    }

    @some_func() {
    }
    "#;
    let prog = parse_from_string(PROGRAM);
    let cfg = program_to_cfg(&prog);
    let rvsdg = cfg_to_rvsdg(&cfg).unwrap();

    let mut expected = RvsdgTest::default();
    let c = expected.lit_bool(true);
    let some_func = expected.void_function("some_func", &[Operand::Arg(0)]);
    let other_func = expected.void_function("other_func", &[Operand::Arg(0)]);
    let gamma = expected.gamma(c, &[Operand::Arg(0)], &[&[other_func], &[some_func]]);
    let res = Operand::Project(0, gamma);

    assert!(deep_equal(
        &expected.into_function(0, None, res),
        &rvsdg.functions[0]
    ));
}

#[test]
fn rvsdg_unstructured() {
    const PROGRAM: &str = r#"@main(): int {
        x: int = const 4;
        a_cond: bool = lt x x;
        br a_cond .B .C;
      .B:
        a: int = const 1;
        b_cond: bool = lt x a;
        x: int = add x a;
        br b_cond .C .D;
      .C:
        jmp .B;
      .D:
        ret x;
      }"#;
    let prog = parse_from_string(PROGRAM);
    let cfg = program_to_cfg(&prog);
    let rvsdg = &cfg_to_rvsdg(&cfg).unwrap().functions[0];

    // It's hard to write a useful test that's more than just a "change
    // detector" here. In this case, the function is not computing anything
    // meaningful, but we know it should have the following properties:
    //
    // 1. A theta node: .B and .C form a cycle.
    // 2. A gamma node, as there is a join point in .B for the value of `x`
    // (whether the predecessor is .B or the entry block).
    assert!(rvsdg.result.is_some());
    assert!(search_for(rvsdg, |body| matches!(
        body,
        RvsdgBody::Theta { .. }
    )));
    assert!(search_for(rvsdg, |body| matches!(
        body,
        RvsdgBody::Gamma { .. }
    )))
}

#[test]
fn rvsdg_basic_odd_branch() {
    // Bril program summing the numbers from 1 to n, multiplying by 2 if that
    // value is larger than 5. This gives us a theta node and a gamma
    // node, with the gamma requiring branch restructuring.
    const PROGRAM: &str = r#"
 @main(n: int): int {
    res: int = const 0;
    i: int = const 0;
 .loop:
    one: int = const 1;
    res: int = add res i;
    i: int = add i one;
    loop_cond: bool = lt i n;
    br loop_cond .loop .tail;
 .tail:
   five: int = const 5;
   rescale_cond: bool = lt res five;
   br rescale_cond .rescale .exit;
 .rescale:
   two: int = const 2;
   res: int = mul res two;
 .exit:
  ret res;
}"#;

    // construct expected program
    let mut expected = RvsdgTest::default();
    let state = Operand::Arg(1);
    let zero = expected.lit_int(0);
    let one = expected.lit_int(1);
    let two = expected.lit_int(2);
    let five = expected.lit_int(5);

    // loop variables
    let res = Operand::Arg(1);
    let i = Operand::Arg(2);
    let n = Operand::Arg(3);

    let ip1 = expected.add(i, one, Type::Int);
    let rpi = expected.add(res, i, Type::Int);
    let pred = expected.lt(ip1, n);
    let theta = expected.theta(
        pred,
        &[state, zero, zero, Operand::Arg(0)],
        &[
            Operand::Arg(0), // state = state
            rpi,             // res = res + i
            ip1,             // i = i + 1
            n,               // n = n
        ],
    );
    let state = Operand::Project(0, theta);
    let res = Operand::Project(1, theta);
    let pred = expected.lt(res, five);
    let mul2 = expected.mul(Operand::Arg(1), two, Type::Int);
    let gamma = expected.gamma(
        pred,
        &[state, res],
        &[
            &[Operand::Arg(0), Operand::Arg(1)],
            &[Operand::Arg(0), mul2],
        ],
    );
    let expected = expected.into_function(
        1,
        Some(Operand::Project(1, gamma)),
        Operand::Project(0, gamma),
    );

    // test correctness of RVSDGs converted from CFG
    let prog = parse_from_string(PROGRAM);
    let cfg = program_to_cfg(&prog);
    let actual = &cfg_to_rvsdg(&cfg).unwrap().functions[0];
    assert!(deep_equal(&expected, actual));
}

#[test]
fn rvsdg_odd_branch_egg_roundtrip() {
    // Bril program summing the numbers from 1 to n, multiplying by 2 if that
    // value is larger than 5. This gives us a theta node and a gamma
    // node, with the gamma requiring branch restructuring.
    const PROGRAM: &str = r#"
 @main(n: int): int {
    res: int = const 0;
    i: int = const 0;
 .loop:
    one: int = const 1;
    res: int = add res i;
    i: int = add i one;
    loop_cond: bool = lt i n;
    br loop_cond .loop .tail;
 .tail:
   five: int = const 5;
   rescale_cond: bool = lt res five;
   br rescale_cond .rescale .exit;
 .rescale:
   two: int = const 2;
   res: int = mul res two;
 .exit:
  ret res;
}"#;

    // construct expected program
    let mut expected = RvsdgTest::default();
    let state = Operand::Arg(1);
    let zero = expected.lit_int(0);
    let one = expected.lit_int(1);
    let two = expected.lit_int(2);
    let five = expected.lit_int(5);

    // loop variables
    let res = Operand::Arg(1);
    let i = Operand::Arg(2);
    let n = Operand::Arg(3);

    let ip1 = expected.add(i, one, Type::Int);
    let rpi = expected.add(res, i, Type::Int);
    let pred = expected.lt(ip1, n);
    let theta = expected.theta(
        pred,
        &[state, zero, zero, Operand::Arg(0)],
        &[
            Operand::Arg(0), // state = state
            rpi,             // res = res + i
            ip1,             // i = i + 1
            n,               // n = n
        ],
    );
    let state = Operand::Project(0, theta);
    let res = Operand::Project(1, theta);
    let pred = expected.lt(res, five);
    let mul2 = expected.mul(Operand::Arg(1), two, Type::Int);
    let gamma = expected.gamma(
        pred,
        &[state, res],
        &[
            &[Operand::Arg(0), Operand::Arg(1)],
            &[Operand::Arg(0), mul2],
        ],
    );
    let expected = expected.into_function(
        1,
        Some(Operand::Project(1, gamma)),
        Operand::Project(0, gamma),
    );

    // test correctness of RVSDGs converted from CFG
    let prog = parse_from_string(PROGRAM);
    let cfg = program_to_cfg(&prog);
    let actual = &cfg_to_rvsdg(&cfg).unwrap().functions[0];
    assert!(deep_equal(&expected, actual));

    // test equalties of egglog programs generated by RVSDG
    let (actual_state, actual_result) = match actual.to_egglog_expr() {
        EgglogFunctionResult::StateOnly(_) => panic!("expected state and value"),
        EgglogFunctionResult::StateAndValue { state, value } => (state, value),
    };

    let actual_result_command = egglog::ast::Command::Action(egglog::ast::Action::Let(
        "actual-result".into(),
        actual_result.clone(),
    ));
    let actual_state_command = egglog::ast::Command::Action(egglog::ast::Action::Let(
        "actual-state".into(),
        actual_state.clone(),
    ));
    const EGGLOG_PROGRAM: &str = r#"
    (let loop
        (Theta
              (Node (PureOp (lt (BoolT) (Node (PureOp (add (IntT) (Arg 2)
                                                   (Node (PureOp (Const (IntT)
                                                                        (const)
                                                                        (Num 1)))))))
                                (Arg 3))))
              (vec-of (Arg 1)
                      (Node (PureOp (Const (IntT) (const) (Num 0))))
                      (Node (PureOp (Const (IntT) (const) (Num 0))))
                      (Arg 0))
              (vec-of (Arg 0)
                      (Node (PureOp (add (IntT) (Arg 1) (Arg 2))))
                      (Node (PureOp (add (IntT) (Arg 2)
                                         (Node (PureOp (Const (IntT) (const) (Num 1)))))))
                      (Arg 3))))
    (let rescaled 
        (Gamma
         (Node
          (PureOp
           (lt (BoolT) (Project 1 loop)
               (Node (PureOp (Const (IntT) (const) (Num 5)))))))
         (vec-of
          (Project 0 loop)
          (Project 1 loop))
         (vec-of (VO (vec-of (Arg 0) (Arg 1)))
                 (VO (vec-of (Arg 0)
                             (Node (PureOp (mul (IntT) (Arg 1)
                                                (Node (PureOp (Const (IntT)
                                                                     (const)
                                                                     (Num 2))))))))))))
    (let expected-result (Project 0 rescaled))
    (let expected-state (Project 1 rescaled))
    "#;
    let mut egraph = new_rvsdg_egraph();
    egraph.parse_and_run_program(EGGLOG_PROGRAM).unwrap();
    // this is weird; shouldn't stop be an optional argument
    egraph
        .process_commands(
            vec![actual_result_command, actual_state_command],
            egglog::CompilerPassStop::All,
        )
        .unwrap();
    egraph
        .parse_and_run_program("(check (= expected-result actual-result))")
        .unwrap();
    egraph
        .parse_and_run_program("(check (= expected-state actual-state))")
        .unwrap();

    // test correctness of RVSDG from egglog
    let actual = RvsdgFunction::egglog_expr_to_function(
        &EgglogFunctionResult::StateAndValue {
            state: actual_state,
            value: actual_result,
        },
        1,
    );
    assert!(deep_equal(&expected, &actual));
}

fn search_for(f: &RvsdgFunction, mut pred: impl FnMut(&RvsdgBody) -> bool) -> bool {
    fn search_op(
        f: &RvsdgFunction,
        op: &Operand,
        pred: &mut impl FnMut(&RvsdgBody) -> bool,
    ) -> bool {
        match op {
            Operand::Arg(_) => false,
            Operand::Id(x) | Operand::Project(_, x) => search_node(f, &f.nodes[*x], pred),
        }
    }
    fn search_node(
        f: &RvsdgFunction,
        node: &RvsdgBody,
        pred: &mut impl FnMut(&RvsdgBody) -> bool,
    ) -> bool {
        if pred(node) {
            return true;
        }
        match node {
            RvsdgBody::BasicOp(x) => match x {
                Expr::Op(_, args, _) | Expr::Call(_, args, _, _) | Expr::Print(args) => {
                    args.iter().any(|arg| search_op(f, arg, pred))
                }
                Expr::Const(_, _, _) => false,
            },
            RvsdgBody::Gamma {
                pred: p,
                inputs,
                outputs,
            } => {
                search_op(f, p, pred)
                    || inputs.iter().any(|arg| search_op(f, arg, pred))
                    || outputs
                        .iter()
                        .any(|outs| outs.iter().any(|arg| search_op(f, arg, pred)))
            }
            RvsdgBody::Theta {
                pred: p,
                inputs,
                outputs,
            } => {
                search_op(f, p, pred)
                    || inputs.iter().any(|arg| search_op(f, arg, pred))
                    || outputs.iter().any(|arg| search_op(f, arg, pred))
            }
        }
    }
    if search_op(f, &f.state, &mut pred) {
        return true;
    }
    f.result
        .as_ref()
        .map(|res| search_op(f, res, &mut pred))
        .unwrap_or(false)
}

/// We don't want to commit to the order in which nodes are laid out, so we do a
/// DFS to check if two functions are equal for the purposes of testing.
fn deep_equal(f1: &RvsdgFunction, f2: &RvsdgFunction) -> bool {
    if f1.n_args != f2.n_args {
        return false;
    }

    fn ops_equal(o1: &Operand, o2: &Operand, f1: &RvsdgFunction, f2: &RvsdgFunction) -> bool {
        match (o1, o2) {
            (Operand::Arg(x), Operand::Arg(y)) => x == y,
            (Operand::Project(p1, l), Operand::Project(p2, r)) => {
                p1 == p2 && ids_equal(*l, *r, f1, f2)
            }
            (Operand::Id(l), Operand::Id(r))
            | (Operand::Project(0, l), Operand::Id(r))
            | (Operand::Id(l), Operand::Project(0, r)) => ids_equal(*l, *r, f1, f2),
            (Operand::Arg(_), Operand::Id(_))
            | (Operand::Arg(_), Operand::Project(_, _))
            | (Operand::Id(_), Operand::Arg(_))
            | (Operand::Project(_, _), Operand::Arg(_))
            | (Operand::Project(_, _), Operand::Id(_))
            | (Operand::Id(_), Operand::Project(_, _)) => false,
        }
    }

    fn all_equal(
        ops1: &[Operand],
        ops2: &[Operand],
        f1: &RvsdgFunction,
        f2: &RvsdgFunction,
    ) -> bool {
        ops1.len() == ops2.len()
            && ops1
                .iter()
                .zip(ops2.iter())
                .all(|(l, r)| ops_equal(l, r, f1, f2))
    }

    fn ids_equal(i1: Id, i2: Id, f1: &RvsdgFunction, f2: &RvsdgFunction) -> bool {
        match (&f1.nodes[i1], &f2.nodes[i2]) {
            (RvsdgBody::BasicOp(l), RvsdgBody::BasicOp(r)) => match (l, r) {
                (Expr::Op(vo1, as1, ty1), Expr::Op(vo2, as2, ty2)) => {
                    vo1 == vo2 && all_equal(as1, as2, f1, f2) && ty1 == ty2
                }
                (Expr::Call(func1, as1, n1, ty1), Expr::Call(func2, as2, n2, ty2)) => {
                    func1 == func2 && n1 == n2 && all_equal(as1, as2, f1, f2) && ty1 == ty2
                }
                (Expr::Const(c1, ty1, lit1), Expr::Const(c2, ty2, lit2)) => {
                    c1 == c2 && ty1 == ty2 && lit1 == lit2
                }
                (Expr::Print(as1), Expr::Print(as2)) => all_equal(as1, as2, f1, f2),
                (Expr::Call(_, _, _, _), Expr::Op(_, _, _))
                | (Expr::Call(_, _, _, _), Expr::Const(_, _, _))
                | (Expr::Call(_, _, _, _), Expr::Print(_))
                | (Expr::Const(_, _, _), Expr::Call(_, _, _, _))
                | (Expr::Const(_, _, _), Expr::Op(_, _, _))
                | (Expr::Const(_, _, _), Expr::Print(_))
                | (Expr::Op(_, _, _), Expr::Call(_, _, _, _))
                | (Expr::Op(_, _, _), Expr::Const(_, _, _))
                | (Expr::Op(_, _, _), Expr::Print(_))
                | (Expr::Print(_), Expr::Call(_, _, _, _))
                | (Expr::Print(_), Expr::Const(_, _, _))
                | (Expr::Print(_), Expr::Op(_, _, _)) => false,
            },
            (
                RvsdgBody::Theta {
                    pred: p1,
                    inputs: is1,
                    outputs: os1,
                },
                RvsdgBody::Theta {
                    pred: p2,
                    inputs: is2,
                    outputs: os2,
                },
            ) => {
                ops_equal(p1, p2, f1, f2)
                    && all_equal(is1, is2, f1, f2)
                    && all_equal(os1, os2, f1, f2)
            }
            (
                RvsdgBody::Gamma {
                    pred: p1,
                    inputs: is1,
                    outputs: os1,
                },
                RvsdgBody::Gamma {
                    pred: p2,
                    inputs: is2,
                    outputs: os2,
                },
            ) => {
                if !ops_equal(p1, p2, f1, f2) || !all_equal(is1, is2, f1, f2) {
                    return false;
                }
                os1.len() == os2.len()
                    && os1
                        .iter()
                        .zip(os2.iter())
                        .all(|(l, r)| all_equal(l, r, f1, f2))
            }
            (RvsdgBody::BasicOp(_), RvsdgBody::Gamma { .. })
            | (RvsdgBody::BasicOp(_), RvsdgBody::Theta { .. })
            | (RvsdgBody::Gamma { .. }, RvsdgBody::Theta { .. })
            | (RvsdgBody::Gamma { .. }, RvsdgBody::BasicOp(_))
            | (RvsdgBody::Theta { .. }, RvsdgBody::BasicOp(_))
            | (RvsdgBody::Theta { .. }, RvsdgBody::Gamma { .. }) => false,
        }
    }

    if !ops_equal(&f1.state, &f2.state, f1, f2) {
        return false;
    }

    match (&f1.result, &f2.result) {
        (Some(o1), Some(o2)) => ops_equal(o1, o2, f1, f2),
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
    }
}
