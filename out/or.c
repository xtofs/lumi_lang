#include "lumi_runtime.h"

static Value* or;

static Value* _fn_1(Value* _env, Value* _arg);

static Value* _fn_2(Value* _env, Value* _arg);

/* Lumi _fn_2:
 *   dup(a); if a
 *   then drop(b); a
 *   else drop(a); b
 */
static Value* _fn_2(Value* _env, Value* _arg) {
    Value* a = closure_cap(_env, 0);  /* captured a */
    rc_inc(a);
    rc_dec(_env);  /* release closure env */
    Value* b = _arg;
    rc_inc(a);  /* dup a */
    int _t1 = lumi_is_true(a);
    rc_dec(a);  /* consume condition */
    Value* _t2;
    if (_t1) {
        rc_dec(b);  /* drop b */
        _t2 = a;
    } else {
        rc_dec(a);  /* drop a */
        _t2 = b;
    }
    return _t2;
}

/* Lumi _fn_1:
 *   λ[a] b =>
 *     dup(a); if a
 *     then drop(b); a
 *     else drop(a); b
 */
static Value* _fn_1(Value* _env, Value* _arg) {
    rc_dec(_env);  /* release closure env */
    Value* a = _arg;
    /* declaration */
    Value* _t1 = alloc_closure(_fn_2, 1, a);

    return _t1;
}

/* Lumi or:
 *   λa =>
 *     λ[a] b =>
 *       dup(a); if a
 *       then drop(b); a
 *       else drop(a); b
 */
Value* lumi_or(void) {
    /* declaration */
    Value* _t1 = alloc_closure(_fn_1, 0);

    return _t1;
}

/* Lumi main:
 *   let _sl3 =
 *     print("or True  True  = ")
 *   in
 *     let _r3 =
 *       ((or true) true)
 *     in
 *       let _sv3 =
 *         print(_r3)
 *       in
 *         let _sn3 =
 *           print_nl()
 *         in
 *           let _sl2 =
 *             print("or True  False = ")
 *           in
 *             let _r2 =
 *               ((or true) false)
 *             in
 *               let _sv2 =
 *                 print(_r2)
 *               in
 *                 let _sn2 =
 *                   print_nl()
 *                 in
 *                   let _sl1 =
 *                     print("or False True  = ")
 *                   in
 *                     let _r1 =
 *                       ((or false) true)
 *                     in
 *                       let _sv1 =
 *                         print(_r1)
 *                       in
 *                         let _sn1 =
 *                           print_nl()
 *                         in
 *                           let _sl0 =
 *                             print("or False False = ")
 *                           in
 *                             let _r0 =
 *                               ((or false) false)
 *                             in
 *                               let _sv0 =
 *                                 print(_r0)
 *                               in
 *                                 let _sn0 =
 *                                   print_nl()
 *                                 in
 *                                   drop(_sl3); drop(_sv3); drop(_sn3); drop(_sl2); drop(_sv2); drop(_sn2); drop(_sl1); drop(_sv1); drop(_sn1); drop(_sl0); drop(_sv0); drop(_sn0); ()
 */
Value* lumi_main(void) {
    Value* _t1 = lumi_str("or True  True  = ");
    Value* _t2 = print(_t1);
    Value* _sl3 = _t2;
    Value* _t3 = apply(or, lumi_bool(1));
    Value* _t4 = apply(_t3, lumi_bool(1));
    Value* _r3 = _t4;
    Value* _t5 = print(_r3);
    Value* _sv3 = _t5;
    Value* _t6 = print_nl();
    Value* _sn3 = _t6;
    Value* _t7 = lumi_str("or True  False = ");
    Value* _t8 = print(_t7);
    Value* _sl2 = _t8;
    Value* _t9 = apply(or, lumi_bool(1));
    Value* _t10 = apply(_t9, lumi_bool(0));
    Value* _r2 = _t10;
    Value* _t11 = print(_r2);
    Value* _sv2 = _t11;
    Value* _t12 = print_nl();
    Value* _sn2 = _t12;
    Value* _t13 = lumi_str("or False True  = ");
    Value* _t14 = print(_t13);
    Value* _sl1 = _t14;
    Value* _t15 = apply(or, lumi_bool(0));
    Value* _t16 = apply(_t15, lumi_bool(1));
    Value* _r1 = _t16;
    Value* _t17 = print(_r1);
    Value* _sv1 = _t17;
    Value* _t18 = print_nl();
    Value* _sn1 = _t18;
    Value* _t19 = lumi_str("or False False = ");
    Value* _t20 = print(_t19);
    Value* _sl0 = _t20;
    Value* _t21 = apply(or, lumi_bool(0));
    Value* _t22 = apply(_t21, lumi_bool(0));
    Value* _r0 = _t22;
    Value* _t23 = print(_r0);
    Value* _sv0 = _t23;
    Value* _t24 = print_nl();
    Value* _sn0 = _t24;
    rc_dec(_sl3);  /* drop _sl3 */
    rc_dec(_sv3);  /* drop _sv3 */
    rc_dec(_sn3);  /* drop _sn3 */
    rc_dec(_sl2);  /* drop _sl2 */
    rc_dec(_sv2);  /* drop _sv2 */
    rc_dec(_sn2);  /* drop _sn2 */
    rc_dec(_sl1);  /* drop _sl1 */
    rc_dec(_sv1);  /* drop _sv1 */
    rc_dec(_sn1);  /* drop _sn1 */
    rc_dec(_sl0);  /* drop _sl0 */
    rc_dec(_sv0);  /* drop _sv0 */
    rc_dec(_sn0);  /* drop _sn0 */
    return lumi_unit();
}

int main(void) {
    lumi_runtime_init();
    or = lumi_global(lumi_or());
    
    Value* _result = lumi_main();
    rc_dec(_result);
    
    lumi_release_global(or);
    return 0;
}
