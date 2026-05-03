#include "lumi_runtime.h"

static Value* inc_list;

static Value* _fn_1(Value* _env, Value* _arg);

/* Lumi _fn_1:
 *   match xs
 *     | Nil() [reuse: reuse_xs] =>
 *       Nil() [reuse: reuse_xs]
 *     | Cons(h, t) [reuse: reuse_xs] =>
 *       Cons(int_add(h, lumi_int1()), (inc_list t)) [reuse: reuse_xs]
 */
static Value* _fn_1(Value* _env, Value* _arg) {
    rc_dec(_env);  /* release closure env */
    Value* xs = _arg;
    Value* _t1;
    switch (tag_of(xs)) {
        case TAG_NIL: {
            ReuseToken reuse_xs = try_reuse(xs);/* RC==1 -> recycle; RC>1 -> NULL */
            Value* _t2 = reuse_con(reuse_xs, TAG_NIL, 0);/* Perceus: reuse if RC==1 */
            _t1 = _t2;
            break;
        }
        case TAG_CONS: {
            Value* h = field(xs, 0);  /* field 0 */
            rc_inc(h);
            Value* t = field(xs, 1);  /* field 1 */
            rc_inc(t);
            ReuseToken reuse_xs = try_reuse(xs);/* RC==1 -> recycle; RC>1 -> NULL */
            Value* _t3 = lumi_int1();
            Value* _t4 = int_add(h, _t3);
            Value* _t5 = apply(inc_list, t);
            Value* _t6 = reuse_con(reuse_xs, TAG_CONS, 2);/* Perceus: reuse if RC==1 */
            set_field(_t6, 0, _t4);
            set_field(_t6, 1, _t5);
            _t1 = _t6;
            break;
        }
        default: lumi_panic("unmatched"); break;
    }
    return _t1;
}

/* Lumi inc_list:
 *   λxs =>
 *     match xs
 *       | Nil() [reuse: reuse_xs] =>
 *         Nil() [reuse: reuse_xs]
 *       | Cons(h, t) [reuse: reuse_xs] =>
 *         Cons(int_add(h, lumi_int1()), (inc_list t)) [reuse: reuse_xs]
 */
Value* lumi_inc_list(void) {
    /* declaration */
    Value* _t1 = alloc_closure(_fn_1, 0);

    return _t1;
}

/* Lumi main:
 *   let _inp =
 *     Cons(0, Cons(1, Cons(2, Cons(3, Nil()))))
 *   in
 *     let _l =
 *       print("inc_list [0,1,2,3] = ")
 *     in
 *       let _out =
 *         (inc_list _inp)
 *       in
 *         let _p =
 *           print(_out)
 *         in
 *           drop(_l); drop(_p); print_nl()
 */
Value* lumi_main(void) {
    Value* _t1 = alloc_con(TAG_NIL, 0);
    Value* _t2 = alloc_con(TAG_CONS, 2);
    set_field(_t2, 0, lumi_int(3));
    set_field(_t2, 1, _t1);
    Value* _t3 = alloc_con(TAG_CONS, 2);
    set_field(_t3, 0, lumi_int(2));
    set_field(_t3, 1, _t2);
    Value* _t4 = alloc_con(TAG_CONS, 2);
    set_field(_t4, 0, lumi_int(1));
    set_field(_t4, 1, _t3);
    Value* _t5 = alloc_con(TAG_CONS, 2);
    set_field(_t5, 0, lumi_int(0));
    set_field(_t5, 1, _t4);
    Value* _inp = _t5;
    Value* _t6 = lumi_str("inc_list [0,1,2,3] = ");
    Value* _t7 = print(_t6);
    Value* _l = _t7;
    Value* _t8 = apply(inc_list, _inp);
    Value* _out = _t8;
    Value* _t9 = print(_out);
    Value* _p = _t9;
    rc_dec(_l);  /* drop _l */
    rc_dec(_p);  /* drop _p */
    Value* _t10 = print_nl();
    return _t10;
}

int main(void) {
    lumi_runtime_init();
    inc_list = lumi_global(lumi_inc_list());
    
    Value* _result = lumi_main();
    rc_dec(_result);
    
    lumi_release_global(inc_list);
    return 0;
}
