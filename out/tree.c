#include "lumi_runtime.h"

static Value* tree;
static Value* sum_tree;

static Value* _fn_1(Value* _env, Value* _arg);

/* Lumi _fn_1:
 *   dup(n); if int_eq(n, lumi_int0())
 *   then drop(n); Zero()
 *   else dup(n); Cons((tree int_sub(n, lumi_int1())), (tree int_sub(n, lumi_int1())))
 */
static Value* _fn_1(Value* _env, Value* _arg) {
    rc_dec(_env);  /* release closure env */
    Value* n = _arg;
    rc_inc(n);  /* dup n */
    Value* _t1 = lumi_int0();
    Value* _t2 = int_eq(n, _t1);
    int _t3 = lumi_is_true(_t2);
    rc_dec(_t2);  /* consume condition */
    Value* _t4;
    if (_t3) {
        rc_dec(n);  /* drop n */
        Value* _t5 = alloc_con(TAG_ZERO, 0);
        _t4 = _t5;
    } else {
        rc_inc(n);  /* dup n */
        Value* _t6 = lumi_int1();
        Value* _t7 = int_sub(n, _t6);
        Value* _t8 = apply(tree, _t7);
        Value* _t9 = lumi_int1();
        Value* _t10 = int_sub(n, _t9);
        Value* _t11 = apply(tree, _t10);
        Value* _t12 = alloc_con(TAG_CONS, 2);
        set_field(_t12, 0, _t8);
        set_field(_t12, 1, _t11);
        _t4 = _t12;
    }
    return _t4;
}

static Value* _fn_2(Value* _env, Value* _arg);

/* Lumi _fn_2:
 *   match t
 *     | Zero() [reuse: reuse_t] =>
 *       1
 *     | Cons(l, r) [reuse: reuse_t] =>
 *       int_add((sum_tree l), (sum_tree r))
 */
static Value* _fn_2(Value* _env, Value* _arg) {
    rc_dec(_env);  /* release closure env */
    Value* t = _arg;
    Value* _t1;
    switch (tag_of(t)) {
        case TAG_ZERO: {
            ReuseToken reuse_t = try_reuse(t);/* RC==1 -> recycle; RC>1 -> NULL */
            _t1 = lumi_int(1);
            break;
        }
        case TAG_CONS: {
            Value* l = field(t, 0);  /* field 0 */
            rc_inc(l);
            Value* r = field(t, 1);  /* field 1 */
            rc_inc(r);
            ReuseToken reuse_t = try_reuse(t);/* RC==1 -> recycle; RC>1 -> NULL */
            Value* _t2 = apply(sum_tree, l);
            Value* _t3 = apply(sum_tree, r);
            Value* _t4 = int_add(_t2, _t3);
            _t1 = _t4;
            break;
        }
        default: lumi_panic("unmatched"); break;
    }
    return _t1;
}

/* Lumi tree:
 *   λn =>
 *     dup(n); if int_eq(n, lumi_int0())
 *     then drop(n); Zero()
 *     else dup(n); Cons((tree int_sub(n, lumi_int1())), (tree int_sub(n, lumi_int1())))
 */
Value* lumi_tree(void) {
    /* declaration */
    Value* _t1 = alloc_closure(_fn_1, 0);

    return _t1;
}

/* Lumi sum_tree:
 *   λt =>
 *     match t
 *       | Zero() [reuse: reuse_t] =>
 *         1
 *       | Cons(l, r) [reuse: reuse_t] =>
 *         int_add((sum_tree l), (sum_tree r))
 */
Value* lumi_sum_tree(void) {
    /* declaration */
    Value* _t1 = alloc_closure(_fn_2, 0);

    return _t1;
}

/* Lumi main:
 *   let _l =
 *     print("sum_tree(tree(24)) = ")
 *   in
 *     let _t =
 *       (tree 24)
 *     in
 *       let _s =
 *         (sum_tree _t)
 *       in
 *         let _ps =
 *           print(_s)
 *         in
 *           drop(_l); drop(_ps); print_nl()
 */
Value* lumi_main(void) {
    Value* _t1 = lumi_str("sum_tree(tree(24)) = ");
    Value* _t2 = print(_t1);
    Value* _l = _t2;
    Value* _t3 = apply(tree, lumi_int(24));
    Value* _t = _t3;
    Value* _t4 = apply(sum_tree, _t);
    Value* _s = _t4;
    Value* _t5 = print(_s);
    Value* _ps = _t5;
    rc_dec(_l);  /* drop _l */
    rc_dec(_ps);  /* drop _ps */
    Value* _t6 = print_nl();
    return _t6;
}

int main(void) {
    lumi_runtime_init();
    tree = lumi_global(lumi_tree());
    sum_tree = lumi_global(lumi_sum_tree());
    
    Value* _result = lumi_main();
    rc_dec(_result);
    
    lumi_release_global(tree);
    lumi_release_global(sum_tree);
    return 0;
}
