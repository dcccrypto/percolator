// Lean compiler output
// Module: ClauseFlow.Core
// Imports: public import Init public meta import Init
#include <lean/lean.h>
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-parameter"
#pragma clang diagnostic ignored "-Wunused-label"
#elif defined(__GNUC__) && !defined(__CLANG__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-label"
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#endif
#ifdef __cplusplus
extern "C" {
#endif
lean_object* l_Nat_reprFast(lean_object*);
lean_object* l_Repr_addAppParen(lean_object*, lean_object*);
uint8_t lean_nat_dec_le(lean_object*, lean_object*);
lean_object* lean_nat_to_int(lean_object*);
lean_object* l_List_reverse___redArg(lean_object*);
lean_object* l_Std_Format_joinSep___at___00Lean_Syntax_formatStxAux_spec__2(lean_object*, lean_object*);
lean_object* lean_string_length(lean_object*);
lean_object* l_instDecidableEqNat___boxed(lean_object*, lean_object*);
lean_object* l_instBEqOfDecidableEq___redArg___lam__0___boxed(lean_object*, lean_object*, lean_object*);
uint8_t l_List_elem___redArg(lean_object*, lean_object*, lean_object*);
uint8_t lean_nat_dec_eq(lean_object*, lean_object*);
lean_object* l_Option_repr___at___00Lean_Meta_instReprConfig__1_repr_spec__0(lean_object*, lean_object*);
lean_object* l_Std_Format_fill(lean_object*);
lean_object* l_List_appendTR___redArg(lean_object*, lean_object*);
uint8_t l_instDecidableEqProd___redArg(lean_object*, lean_object*, lean_object*, lean_object*);
lean_object* lean_mk_empty_array_with_capacity(lean_object*);
lean_object* lean_array_to_list(lean_object*);
lean_object* l_List_foldl___at___00Array_appendList_spec__0___redArg(lean_object*, lean_object*);
lean_object* l_List_get_x3fInternal___redArg(lean_object*, lean_object*);
uint8_t l_instDecidableEqList___redArg(lean_object*, lean_object*, lean_object*);
lean_object* l_List_lengthTR___redArg(lean_object*);
lean_object* l_List_range(lean_object*);
uint8_t l_List_isEmpty___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_eq_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_eq_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ne_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ne_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqTest_decEq(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqTest_decEq___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqTest(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqTest___boxed(lean_object*, lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTest_repr___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 19, .m_capacity = 19, .m_length = 18, .m_data = "ClauseFlow.Test.eq"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTest_repr___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__1_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTest_repr___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__1_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__2_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTest_repr___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 19, .m_capacity = 19, .m_length = 18, .m_data = "ClauseFlow.Test.ne"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__5_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTest_repr___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__6 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__6_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTest_repr___closed__7_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__6_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___closed__7 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__7_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprTest___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprTest_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprTest___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprTest = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTest___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_neg(lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_Test_conflict(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_conflict___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_Test_imp(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_imp___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_holds_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_holds_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableHolds(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableHolds___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableHolds__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableHolds__1___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_bot_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_bot_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_chk_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_chk_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqConcl(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqConcl___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0___lam__0(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0_spec__1_spec__2(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0(lean_object*, lean_object*);
static const lean_string_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 3, .m_capacity = 3, .m_length = 2, .m_data = "[]"};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__0 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__0_value;
static const lean_ctor_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__0_value)}};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1_value;
static const lean_string_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 2, .m_capacity = 2, .m_length = 1, .m_data = "["};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__2 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__2_value;
static const lean_string_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 2, .m_capacity = 2, .m_length = 1, .m_data = ","};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__3 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__3_value;
static const lean_ctor_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__3_value)}};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4_value;
static const lean_ctor_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5_value;
static const lean_string_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 2, .m_capacity = 2, .m_length = 1, .m_data = "]"};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__6 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__6_value;
static lean_once_cell_t lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__7_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__7;
static lean_once_cell_t lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8;
static const lean_ctor_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__2_value)}};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9_value;
static const lean_ctor_object lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__6_value)}};
static const lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10 = (const lean_object*)&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10_value;
LEAN_EXPORT lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Concl.bot"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__1_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Concl.chk"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__2_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__2_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__3_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__3_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__4 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__4_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprConcl___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprConcl_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprConcl___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprConcl = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprConcl___closed__0_value;
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___lam__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___lam__0___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___lam__0___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___closed__0_value;
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqClause(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause___boxed(lean_object*, lean_object*);
static const lean_string_object lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 2, .m_capacity = 2, .m_length = 1, .m_data = "("};
static const lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__0 = (const lean_object*)&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__0_value;
static const lean_string_object lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 2, .m_capacity = 2, .m_length = 1, .m_data = ")"};
static const lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__1 = (const lean_object*)&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__1_value;
static lean_once_cell_t lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__2_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__2;
static lean_once_cell_t lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3;
static const lean_ctor_object lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__0_value)}};
static const lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__4 = (const lean_object*)&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__4_value;
static const lean_ctor_object lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__1_value)}};
static const lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__5 = (const lean_object*)&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__5_value;
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1_spec__2_spec__3(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1_spec__2(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___redArg(lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 3, .m_capacity = 3, .m_length = 2, .m_data = "{ "};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__0_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 3, .m_capacity = 3, .m_length = 2, .m_data = "gs"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__1_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__1_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__2_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__2_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__3_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = " := "};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__4 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__4_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__4_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__3_value),((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__6 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__6_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__7_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__7;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__8_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 2, .m_capacity = 2, .m_length = 1, .m_data = "c"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__8 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__8_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__9_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__8_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__9 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__9_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__10_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__10;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__11_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 3, .m_capacity = 3, .m_length = 2, .m_data = " }"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__11 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__11_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__12_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__12;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__11_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprClause___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprClause_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprClause___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprClause = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprClause___closed__0_value;
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_Concl_isBot(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_isBot___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_hs(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_hs___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ps(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ps___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_havoc_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_havoc_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_write_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_write_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_setD_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_setD_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_copy_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_copy_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpEq_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpEq_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpNe_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpNe_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqStmt_decEq(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqStmt_decEq___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqStmt(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqStmt___boxed(lean_object*, lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 22, .m_capacity = 22, .m_length = 21, .m_data = "ClauseFlow.Stmt.havoc"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__1_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__1_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__2_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 22, .m_capacity = 22, .m_length = 21, .m_data = "ClauseFlow.Stmt.write"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__3_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__3_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__4 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__4_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__4_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__5_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Stmt.setD"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__6 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__6_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__7_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__6_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__7 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__7_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__8_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__7_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__8 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__8_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__9_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Stmt.copy"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__9 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__9_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__10_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__9_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__10 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__10_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__11_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__10_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__11 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__11_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__12_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 22, .m_capacity = 22, .m_length = 21, .m_data = "ClauseFlow.Stmt.cmpEq"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__12 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__12_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__13_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__12_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__13 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__13_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__14_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__13_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__14 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__14_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__15_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 22, .m_capacity = 22, .m_length = 21, .m_data = "ClauseFlow.Stmt.cmpNe"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__15 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__15_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__16_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__15_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__16 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__16_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__17_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__16_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__17 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__17_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprStmt___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprStmt_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprStmt___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprStmt = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprStmt___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_goto_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_goto_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_switch_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_switch_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_call_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_call_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_branch_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_branch_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ret_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ret_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_stop_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_stop_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3_spec__5_spec__7(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3_spec__5(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1_spec__2_spec__4(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1_spec__2(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0___redArg(lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 20, .m_capacity = 20, .m_length = 19, .m_data = "ClauseFlow.Term.ret"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__1_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Term.stop"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__2_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__2_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__3_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Term.goto"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__4 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__4_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__4_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__5_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__5_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__6 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__6_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__7_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 23, .m_capacity = 23, .m_length = 22, .m_data = "ClauseFlow.Term.switch"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__7 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__7_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__8_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__7_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__8 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__8_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__9_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__8_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__9 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__9_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__10_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 21, .m_capacity = 21, .m_length = 20, .m_data = "ClauseFlow.Term.call"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__10 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__10_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__11_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__10_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__11 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__11_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__12_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__11_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__12 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__12_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__13_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 23, .m_capacity = 23, .m_length = 22, .m_data = "ClauseFlow.Term.branch"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__13 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__13_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__14_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__13_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__14 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__14_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__15_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__14_value),((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__15 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__15_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprTerm___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprTerm_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprTerm___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprTerm = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprTerm___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0___lam__0(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0_spec__1_spec__2(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0___redArg(lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 6, .m_capacity = 6, .m_length = 5, .m_data = "stmts"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__1_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__1_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__2_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__2_value),((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__3_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = "term"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__5_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__6 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__6_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprBlockD___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprBlockD_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprBlockD = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprBlockD___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_upd(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_upd___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_lookupCase(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_lookupCase___boxed(lean_object*, lean_object*);
static const lean_string_object lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 6, .m_capacity = 6, .m_length = 5, .m_data = "facts"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__0_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__1_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__1_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__2_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__2_value),((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__3_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 4, .m_capacity = 4, .m_length = 3, .m_data = "cls"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__4 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__4_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__4_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__5_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprAState___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprAState_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprAState___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprAState = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprAState___closed__0_value;
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_inconsistent(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_inconsistent___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_implied(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_implied___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_fired_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_fired_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_fired_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_fired_spec__1___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_fired(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_fired___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_unitLits_spec__0(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_unitLits_spec__0___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_unitLits_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_unitLits_spec__1___boxed(lean_object*, lean_object*, lean_object*);
static const lean_array_object lp_clauseflow_ClauseFlow_unitLits___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_array_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 246}, .m_size = 0, .m_capacity = 0, .m_data = {}};
static const lean_object* lp_clauseflow_ClauseFlow_unitLits___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_unitLits___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_unitLits(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_unitLits___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_closure(lean_object*, lean_object*);
static lean_once_cell_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0;
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__2(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__2___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__4(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__4___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__3(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__3___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_entailsCl(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_entailsCl___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_factCl(lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_le_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_le_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_le(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_le___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_mentions_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_mentions_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_mentions(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_mentions___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__1___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__0(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__0___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_kill(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_kill___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_Concl_restrict_spec__0(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_restrict(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_clobber_spec__0(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_clobber(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameLit(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameLit___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameCl(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameCl___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_swapLit(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_cmpCls_spec__0(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_cmpCls_spec__2(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_cmpCls(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__2(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__2___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tStmt_spec__0(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tStmt_spec__0___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__1(lean_object*, lean_object*, lean_object*);
static const lean_ctor_object lp_clauseflow_ClauseFlow_tStmt___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 0}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_tStmt___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_tStmt___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_tStmt___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_tStmt___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_tStmt___closed__1_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_tStmt(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_tStmts(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_optEdge(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_optEdge___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_knownH_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownH_spec__1(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownH_spec__1___boxed(lean_object*, lean_object*);
static const lean_array_object lp_clauseflow_ClauseFlow_knownH___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_array_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 246}, .m_size = 0, .m_capacity = 0, .m_data = {}};
static const lean_object* lp_clauseflow_ClauseFlow_knownH___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_knownH___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_knownH(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownP_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownP_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_knownP(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_rebase(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_killAll(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_killAll___boxed(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__0(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__0___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__5(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__5___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__2(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__3(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6_spec__6(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6_spec__6___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__4(lean_object*, lean_object*);
static const lean_ctor_object lp_clauseflow_ClauseFlow_tTerm___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_clauseflow_ClauseFlow_tTerm___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_tTerm___closed__0_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_tTerm(lean_object*, lean_object*);
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__0_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 5}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__0_value),((lean_object*)&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__1 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__1_value;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 7, .m_capacity = 7, .m_length = 6, .m_data = "exempt"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__2 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__2_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__2_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__3 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__3_value;
static lean_once_cell_t lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__4_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__4;
static const lean_string_object lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = "init"};
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__5 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__5_value;
static const lean_ctor_object lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 3}, .m_objs = {((lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__5_value)}};
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__6 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__6_value;
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___boxed(lean_object*, lean_object*);
static const lean_closure_object lp_clauseflow_ClauseFlow_instReprGoal___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_clauseflow_ClauseFlow_instReprGoal_repr___boxed, .m_arity = 2, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_clauseflow_ClauseFlow_instReprGoal___closed__0 = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal___closed__0_value;
LEAN_EXPORT const lean_object* lp_clauseflow_ClauseFlow_instReprGoal = (const lean_object*)&lp_clauseflow_ClauseFlow_instReprGoal___closed__0_value;
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_edgeOk(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_edgeOk___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_blockOk_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_blockOk_spec__0___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_blockOk(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_blockOk___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_initState(lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_initState___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_checkCert_spec__0(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_checkCert_spec__0___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_checkCert(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_checkCert___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprTest_repr_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprTest_repr_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instDecidableEqTest_decEq_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instDecidableEqTest_decEq_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_imp_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_imp_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_lookupCase_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_lookupCase_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_unitLits_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_unitLits_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprConcl_repr_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprConcl_repr_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tStmt_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tStmt_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_killAll_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_killAll_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_optEdge_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_optEdge_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tTerm_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tTerm_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_checkCert_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_checkCert_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_edgeOk_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_edgeOk_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_blockOk_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_blockOk_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorIdx(lean_object* v_x_1_){
_start:
{
if (lean_obj_tag(v_x_1_) == 0)
{
lean_object* v___x_2_; 
v___x_2_ = lean_unsigned_to_nat(0u);
return v___x_2_;
}
else
{
lean_object* v___x_3_; 
v___x_3_ = lean_unsigned_to_nat(1u);
return v___x_3_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorIdx___boxed(lean_object* v_x_4_){
_start:
{
lean_object* v_res_5_; 
v_res_5_ = lp_clauseflow_ClauseFlow_Test_ctorIdx(v_x_4_);
lean_dec_ref(v_x_4_);
return v_res_5_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(lean_object* v_t_6_, lean_object* v_k_7_){
_start:
{
lean_object* v_n_8_; lean_object* v___x_9_; 
v_n_8_ = lean_ctor_get(v_t_6_, 0);
lean_inc(v_n_8_);
lean_dec_ref(v_t_6_);
v___x_9_ = lean_apply_1(v_k_7_, v_n_8_);
return v___x_9_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorElim(lean_object* v_motive_10_, lean_object* v_ctorIdx_11_, lean_object* v_t_12_, lean_object* v_h_13_, lean_object* v_k_14_){
_start:
{
lean_object* v___x_15_; 
v___x_15_ = lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(v_t_12_, v_k_14_);
return v___x_15_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ctorElim___boxed(lean_object* v_motive_16_, lean_object* v_ctorIdx_17_, lean_object* v_t_18_, lean_object* v_h_19_, lean_object* v_k_20_){
_start:
{
lean_object* v_res_21_; 
v_res_21_ = lp_clauseflow_ClauseFlow_Test_ctorElim(v_motive_16_, v_ctorIdx_17_, v_t_18_, v_h_19_, v_k_20_);
lean_dec(v_ctorIdx_17_);
return v_res_21_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_eq_elim___redArg(lean_object* v_t_22_, lean_object* v_eq_23_){
_start:
{
lean_object* v___x_24_; 
v___x_24_ = lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(v_t_22_, v_eq_23_);
return v___x_24_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_eq_elim(lean_object* v_motive_25_, lean_object* v_t_26_, lean_object* v_h_27_, lean_object* v_eq_28_){
_start:
{
lean_object* v___x_29_; 
v___x_29_ = lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(v_t_26_, v_eq_28_);
return v___x_29_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ne_elim___redArg(lean_object* v_t_30_, lean_object* v_ne_31_){
_start:
{
lean_object* v___x_32_; 
v___x_32_ = lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(v_t_30_, v_ne_31_);
return v___x_32_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_ne_elim(lean_object* v_motive_33_, lean_object* v_t_34_, lean_object* v_h_35_, lean_object* v_ne_36_){
_start:
{
lean_object* v___x_37_; 
v___x_37_ = lp_clauseflow_ClauseFlow_Test_ctorElim___redArg(v_t_34_, v_ne_36_);
return v___x_37_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqTest_decEq(lean_object* v_x_38_, lean_object* v_x_39_){
_start:
{
if (lean_obj_tag(v_x_38_) == 0)
{
if (lean_obj_tag(v_x_39_) == 0)
{
lean_object* v_n_40_; lean_object* v_n_41_; uint8_t v___x_42_; 
v_n_40_ = lean_ctor_get(v_x_38_, 0);
v_n_41_ = lean_ctor_get(v_x_39_, 0);
v___x_42_ = lean_nat_dec_eq(v_n_40_, v_n_41_);
return v___x_42_;
}
else
{
uint8_t v___x_43_; 
v___x_43_ = 0;
return v___x_43_;
}
}
else
{
if (lean_obj_tag(v_x_39_) == 0)
{
uint8_t v___x_44_; 
v___x_44_ = 0;
return v___x_44_;
}
else
{
lean_object* v_n_45_; lean_object* v_n_46_; uint8_t v___x_47_; 
v_n_45_ = lean_ctor_get(v_x_38_, 0);
v_n_46_ = lean_ctor_get(v_x_39_, 0);
v___x_47_ = lean_nat_dec_eq(v_n_45_, v_n_46_);
return v___x_47_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqTest_decEq___boxed(lean_object* v_x_48_, lean_object* v_x_49_){
_start:
{
uint8_t v_res_50_; lean_object* v_r_51_; 
v_res_50_ = lp_clauseflow_ClauseFlow_instDecidableEqTest_decEq(v_x_48_, v_x_49_);
lean_dec_ref(v_x_49_);
lean_dec_ref(v_x_48_);
v_r_51_ = lean_box(v_res_50_);
return v_r_51_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqTest(lean_object* v_x_52_, lean_object* v_x_53_){
_start:
{
uint8_t v___x_54_; 
v___x_54_ = lp_clauseflow_ClauseFlow_instDecidableEqTest_decEq(v_x_52_, v_x_53_);
return v___x_54_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqTest___boxed(lean_object* v_x_55_, lean_object* v_x_56_){
_start:
{
uint8_t v_res_57_; lean_object* v_r_58_; 
v_res_57_ = lp_clauseflow_ClauseFlow_instDecidableEqTest(v_x_55_, v_x_56_);
lean_dec_ref(v_x_56_);
lean_dec_ref(v_x_55_);
v_r_58_ = lean_box(v_res_57_);
return v_r_58_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3(void){
_start:
{
lean_object* v___x_65_; lean_object* v___x_66_; 
v___x_65_ = lean_unsigned_to_nat(2u);
v___x_66_ = lean_nat_to_int(v___x_65_);
return v___x_66_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4(void){
_start:
{
lean_object* v___x_67_; lean_object* v___x_68_; 
v___x_67_ = lean_unsigned_to_nat(1u);
v___x_68_ = lean_nat_to_int(v___x_67_);
return v___x_68_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr(lean_object* v_x_75_, lean_object* v_prec_76_){
_start:
{
if (lean_obj_tag(v_x_75_) == 0)
{
lean_object* v_n_77_; lean_object* v___x_79_; uint8_t v_isShared_80_; uint8_t v_isSharedCheck_97_; 
v_n_77_ = lean_ctor_get(v_x_75_, 0);
v_isSharedCheck_97_ = !lean_is_exclusive(v_x_75_);
if (v_isSharedCheck_97_ == 0)
{
v___x_79_ = v_x_75_;
v_isShared_80_ = v_isSharedCheck_97_;
goto v_resetjp_78_;
}
else
{
lean_inc(v_n_77_);
lean_dec(v_x_75_);
v___x_79_ = lean_box(0);
v_isShared_80_ = v_isSharedCheck_97_;
goto v_resetjp_78_;
}
v_resetjp_78_:
{
lean_object* v___y_82_; lean_object* v___x_93_; uint8_t v___x_94_; 
v___x_93_ = lean_unsigned_to_nat(1024u);
v___x_94_ = lean_nat_dec_le(v___x_93_, v_prec_76_);
if (v___x_94_ == 0)
{
lean_object* v___x_95_; 
v___x_95_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_82_ = v___x_95_;
goto v___jp_81_;
}
else
{
lean_object* v___x_96_; 
v___x_96_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_82_ = v___x_96_;
goto v___jp_81_;
}
v___jp_81_:
{
lean_object* v___x_83_; lean_object* v___x_84_; lean_object* v___x_86_; 
v___x_83_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTest_repr___closed__2));
v___x_84_ = l_Nat_reprFast(v_n_77_);
if (v_isShared_80_ == 0)
{
lean_ctor_set_tag(v___x_79_, 3);
lean_ctor_set(v___x_79_, 0, v___x_84_);
v___x_86_ = v___x_79_;
goto v_reusejp_85_;
}
else
{
lean_object* v_reuseFailAlloc_92_; 
v_reuseFailAlloc_92_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v_reuseFailAlloc_92_, 0, v___x_84_);
v___x_86_ = v_reuseFailAlloc_92_;
goto v_reusejp_85_;
}
v_reusejp_85_:
{
lean_object* v___x_87_; lean_object* v___x_88_; uint8_t v___x_89_; lean_object* v___x_90_; lean_object* v___x_91_; 
v___x_87_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_87_, 0, v___x_83_);
lean_ctor_set(v___x_87_, 1, v___x_86_);
lean_inc(v___y_82_);
v___x_88_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_88_, 0, v___y_82_);
lean_ctor_set(v___x_88_, 1, v___x_87_);
v___x_89_ = 0;
v___x_90_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_90_, 0, v___x_88_);
lean_ctor_set_uint8(v___x_90_, sizeof(void*)*1, v___x_89_);
v___x_91_ = l_Repr_addAppParen(v___x_90_, v_prec_76_);
return v___x_91_;
}
}
}
}
else
{
lean_object* v_n_98_; lean_object* v___x_100_; uint8_t v_isShared_101_; uint8_t v_isSharedCheck_118_; 
v_n_98_ = lean_ctor_get(v_x_75_, 0);
v_isSharedCheck_118_ = !lean_is_exclusive(v_x_75_);
if (v_isSharedCheck_118_ == 0)
{
v___x_100_ = v_x_75_;
v_isShared_101_ = v_isSharedCheck_118_;
goto v_resetjp_99_;
}
else
{
lean_inc(v_n_98_);
lean_dec(v_x_75_);
v___x_100_ = lean_box(0);
v_isShared_101_ = v_isSharedCheck_118_;
goto v_resetjp_99_;
}
v_resetjp_99_:
{
lean_object* v___y_103_; lean_object* v___x_114_; uint8_t v___x_115_; 
v___x_114_ = lean_unsigned_to_nat(1024u);
v___x_115_ = lean_nat_dec_le(v___x_114_, v_prec_76_);
if (v___x_115_ == 0)
{
lean_object* v___x_116_; 
v___x_116_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_103_ = v___x_116_;
goto v___jp_102_;
}
else
{
lean_object* v___x_117_; 
v___x_117_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_103_ = v___x_117_;
goto v___jp_102_;
}
v___jp_102_:
{
lean_object* v___x_104_; lean_object* v___x_105_; lean_object* v___x_107_; 
v___x_104_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTest_repr___closed__7));
v___x_105_ = l_Nat_reprFast(v_n_98_);
if (v_isShared_101_ == 0)
{
lean_ctor_set_tag(v___x_100_, 3);
lean_ctor_set(v___x_100_, 0, v___x_105_);
v___x_107_ = v___x_100_;
goto v_reusejp_106_;
}
else
{
lean_object* v_reuseFailAlloc_113_; 
v_reuseFailAlloc_113_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v_reuseFailAlloc_113_, 0, v___x_105_);
v___x_107_ = v_reuseFailAlloc_113_;
goto v_reusejp_106_;
}
v_reusejp_106_:
{
lean_object* v___x_108_; lean_object* v___x_109_; uint8_t v___x_110_; lean_object* v___x_111_; lean_object* v___x_112_; 
v___x_108_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_108_, 0, v___x_104_);
lean_ctor_set(v___x_108_, 1, v___x_107_);
lean_inc(v___y_103_);
v___x_109_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_109_, 0, v___y_103_);
lean_ctor_set(v___x_109_, 1, v___x_108_);
v___x_110_ = 0;
v___x_111_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_111_, 0, v___x_109_);
lean_ctor_set_uint8(v___x_111_, sizeof(void*)*1, v___x_110_);
v___x_112_ = l_Repr_addAppParen(v___x_111_, v_prec_76_);
return v___x_112_;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTest_repr___boxed(lean_object* v_x_119_, lean_object* v_prec_120_){
_start:
{
lean_object* v_res_121_; 
v_res_121_ = lp_clauseflow_ClauseFlow_instReprTest_repr(v_x_119_, v_prec_120_);
lean_dec(v_prec_120_);
return v_res_121_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_neg(lean_object* v_x_124_){
_start:
{
if (lean_obj_tag(v_x_124_) == 0)
{
lean_object* v_n_125_; lean_object* v___x_127_; uint8_t v_isShared_128_; uint8_t v_isSharedCheck_132_; 
v_n_125_ = lean_ctor_get(v_x_124_, 0);
v_isSharedCheck_132_ = !lean_is_exclusive(v_x_124_);
if (v_isSharedCheck_132_ == 0)
{
v___x_127_ = v_x_124_;
v_isShared_128_ = v_isSharedCheck_132_;
goto v_resetjp_126_;
}
else
{
lean_inc(v_n_125_);
lean_dec(v_x_124_);
v___x_127_ = lean_box(0);
v_isShared_128_ = v_isSharedCheck_132_;
goto v_resetjp_126_;
}
v_resetjp_126_:
{
lean_object* v___x_130_; 
if (v_isShared_128_ == 0)
{
lean_ctor_set_tag(v___x_127_, 1);
v___x_130_ = v___x_127_;
goto v_reusejp_129_;
}
else
{
lean_object* v_reuseFailAlloc_131_; 
v_reuseFailAlloc_131_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v_reuseFailAlloc_131_, 0, v_n_125_);
v___x_130_ = v_reuseFailAlloc_131_;
goto v_reusejp_129_;
}
v_reusejp_129_:
{
return v___x_130_;
}
}
}
else
{
lean_object* v_n_133_; lean_object* v___x_135_; uint8_t v_isShared_136_; uint8_t v_isSharedCheck_140_; 
v_n_133_ = lean_ctor_get(v_x_124_, 0);
v_isSharedCheck_140_ = !lean_is_exclusive(v_x_124_);
if (v_isSharedCheck_140_ == 0)
{
v___x_135_ = v_x_124_;
v_isShared_136_ = v_isSharedCheck_140_;
goto v_resetjp_134_;
}
else
{
lean_inc(v_n_133_);
lean_dec(v_x_124_);
v___x_135_ = lean_box(0);
v_isShared_136_ = v_isSharedCheck_140_;
goto v_resetjp_134_;
}
v_resetjp_134_:
{
lean_object* v___x_138_; 
if (v_isShared_136_ == 0)
{
lean_ctor_set_tag(v___x_135_, 0);
v___x_138_ = v___x_135_;
goto v_reusejp_137_;
}
else
{
lean_object* v_reuseFailAlloc_139_; 
v_reuseFailAlloc_139_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v_reuseFailAlloc_139_, 0, v_n_133_);
v___x_138_ = v_reuseFailAlloc_139_;
goto v_reusejp_137_;
}
v_reusejp_137_:
{
return v___x_138_;
}
}
}
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_Test_conflict(lean_object* v_x_141_, lean_object* v_x_142_){
_start:
{
if (lean_obj_tag(v_x_141_) == 0)
{
if (lean_obj_tag(v_x_142_) == 0)
{
lean_object* v_n_143_; lean_object* v_n_144_; uint8_t v___x_145_; 
v_n_143_ = lean_ctor_get(v_x_141_, 0);
v_n_144_ = lean_ctor_get(v_x_142_, 0);
v___x_145_ = lean_nat_dec_eq(v_n_143_, v_n_144_);
if (v___x_145_ == 0)
{
uint8_t v___x_146_; 
v___x_146_ = 1;
return v___x_146_;
}
else
{
uint8_t v___x_147_; 
v___x_147_ = 0;
return v___x_147_;
}
}
else
{
lean_object* v_n_148_; lean_object* v_n_149_; uint8_t v___x_150_; 
v_n_148_ = lean_ctor_get(v_x_141_, 0);
v_n_149_ = lean_ctor_get(v_x_142_, 0);
v___x_150_ = lean_nat_dec_eq(v_n_148_, v_n_149_);
return v___x_150_;
}
}
else
{
if (lean_obj_tag(v_x_142_) == 0)
{
lean_object* v_n_151_; lean_object* v_n_152_; uint8_t v___x_153_; 
v_n_151_ = lean_ctor_get(v_x_141_, 0);
v_n_152_ = lean_ctor_get(v_x_142_, 0);
v___x_153_ = lean_nat_dec_eq(v_n_151_, v_n_152_);
return v___x_153_;
}
else
{
uint8_t v___x_154_; 
v___x_154_ = 0;
return v___x_154_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_conflict___boxed(lean_object* v_x_155_, lean_object* v_x_156_){
_start:
{
uint8_t v_res_157_; lean_object* v_r_158_; 
v_res_157_ = lp_clauseflow_ClauseFlow_Test_conflict(v_x_155_, v_x_156_);
lean_dec_ref(v_x_156_);
lean_dec_ref(v_x_155_);
v_r_158_ = lean_box(v_res_157_);
return v_r_158_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_Test_imp(lean_object* v_x_159_, lean_object* v_x_160_){
_start:
{
if (lean_obj_tag(v_x_159_) == 0)
{
if (lean_obj_tag(v_x_160_) == 0)
{
lean_object* v_n_161_; lean_object* v_n_162_; uint8_t v___x_163_; 
v_n_161_ = lean_ctor_get(v_x_159_, 0);
v_n_162_ = lean_ctor_get(v_x_160_, 0);
v___x_163_ = lean_nat_dec_eq(v_n_161_, v_n_162_);
return v___x_163_;
}
else
{
lean_object* v_n_164_; lean_object* v_n_165_; uint8_t v___x_166_; 
v_n_164_ = lean_ctor_get(v_x_159_, 0);
v_n_165_ = lean_ctor_get(v_x_160_, 0);
v___x_166_ = lean_nat_dec_eq(v_n_164_, v_n_165_);
if (v___x_166_ == 0)
{
uint8_t v___x_167_; 
v___x_167_ = 1;
return v___x_167_;
}
else
{
uint8_t v___x_168_; 
v___x_168_ = 0;
return v___x_168_;
}
}
}
else
{
if (lean_obj_tag(v_x_160_) == 0)
{
uint8_t v___x_169_; 
v___x_169_ = 0;
return v___x_169_;
}
else
{
lean_object* v_n_170_; lean_object* v_n_171_; uint8_t v___x_172_; 
v_n_170_ = lean_ctor_get(v_x_159_, 0);
v_n_171_ = lean_ctor_get(v_x_160_, 0);
v___x_172_ = lean_nat_dec_eq(v_n_170_, v_n_171_);
return v___x_172_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Test_imp___boxed(lean_object* v_x_173_, lean_object* v_x_174_){
_start:
{
uint8_t v_res_175_; lean_object* v_r_176_; 
v_res_175_ = lp_clauseflow_ClauseFlow_Test_imp(v_x_173_, v_x_174_);
lean_dec_ref(v_x_174_);
lean_dec_ref(v_x_173_);
v_r_176_ = lean_box(v_res_175_);
return v_r_176_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_holds_match__1_splitter___redArg(lean_object* v_x_177_, lean_object* v_x_178_, lean_object* v_h__1_179_, lean_object* v_h__2_180_){
_start:
{
if (lean_obj_tag(v_x_177_) == 0)
{
lean_object* v_n_181_; lean_object* v___x_182_; 
lean_dec(v_h__2_180_);
v_n_181_ = lean_ctor_get(v_x_177_, 0);
lean_inc(v_n_181_);
lean_dec_ref_known(v_x_177_, 1);
v___x_182_ = lean_apply_2(v_h__1_179_, v_n_181_, v_x_178_);
return v___x_182_;
}
else
{
lean_object* v_n_183_; lean_object* v___x_184_; 
lean_dec(v_h__1_179_);
v_n_183_ = lean_ctor_get(v_x_177_, 0);
lean_inc(v_n_183_);
lean_dec_ref_known(v_x_177_, 1);
v___x_184_ = lean_apply_2(v_h__2_180_, v_n_183_, v_x_178_);
return v___x_184_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_holds_match__1_splitter(lean_object* v_motive_185_, lean_object* v_x_186_, lean_object* v_x_187_, lean_object* v_h__1_188_, lean_object* v_h__2_189_){
_start:
{
if (lean_obj_tag(v_x_186_) == 0)
{
lean_object* v_n_190_; lean_object* v___x_191_; 
lean_dec(v_h__2_189_);
v_n_190_ = lean_ctor_get(v_x_186_, 0);
lean_inc(v_n_190_);
lean_dec_ref_known(v_x_186_, 1);
v___x_191_ = lean_apply_2(v_h__1_188_, v_n_190_, v_x_187_);
return v___x_191_;
}
else
{
lean_object* v_n_192_; lean_object* v___x_193_; 
lean_dec(v_h__1_188_);
v_n_192_ = lean_ctor_get(v_x_186_, 0);
lean_inc(v_n_192_);
lean_dec_ref_known(v_x_186_, 1);
v___x_193_ = lean_apply_2(v_h__2_189_, v_n_192_, v_x_187_);
return v___x_193_;
}
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableHolds(lean_object* v_t_194_, lean_object* v_v_195_){
_start:
{
if (lean_obj_tag(v_t_194_) == 0)
{
lean_object* v_n_196_; uint8_t v___x_197_; 
v_n_196_ = lean_ctor_get(v_t_194_, 0);
v___x_197_ = lean_nat_dec_eq(v_v_195_, v_n_196_);
return v___x_197_;
}
else
{
lean_object* v_n_198_; uint8_t v___x_199_; 
v_n_198_ = lean_ctor_get(v_t_194_, 0);
v___x_199_ = lean_nat_dec_eq(v_v_195_, v_n_198_);
if (v___x_199_ == 0)
{
uint8_t v___x_200_; 
v___x_200_ = 1;
return v___x_200_;
}
else
{
uint8_t v___x_201_; 
v___x_201_ = 0;
return v___x_201_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableHolds___boxed(lean_object* v_t_202_, lean_object* v_v_203_){
_start:
{
uint8_t v_res_204_; lean_object* v_r_205_; 
v_res_204_ = lp_clauseflow_ClauseFlow_instDecidableHolds(v_t_202_, v_v_203_);
lean_dec(v_v_203_);
lean_dec_ref(v_t_202_);
v_r_205_ = lean_box(v_res_204_);
return v_r_205_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableHolds__1(lean_object* v_env_206_, lean_object* v_p_207_){
_start:
{
lean_object* v_fst_208_; lean_object* v_snd_209_; lean_object* v___x_210_; uint8_t v___x_211_; 
v_fst_208_ = lean_ctor_get(v_p_207_, 0);
lean_inc(v_fst_208_);
v_snd_209_ = lean_ctor_get(v_p_207_, 1);
lean_inc(v_snd_209_);
lean_dec_ref(v_p_207_);
v___x_210_ = lean_apply_1(v_env_206_, v_fst_208_);
v___x_211_ = lp_clauseflow_ClauseFlow_instDecidableHolds(v_snd_209_, v___x_210_);
lean_dec(v___x_210_);
lean_dec(v_snd_209_);
return v___x_211_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableHolds__1___boxed(lean_object* v_env_212_, lean_object* v_p_213_){
_start:
{
uint8_t v_res_214_; lean_object* v_r_215_; 
v_res_214_ = lp_clauseflow_ClauseFlow_instDecidableHolds__1(v_env_212_, v_p_213_);
v_r_215_ = lean_box(v_res_214_);
return v_r_215_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorIdx(lean_object* v_x_216_){
_start:
{
if (lean_obj_tag(v_x_216_) == 0)
{
lean_object* v___x_217_; 
v___x_217_ = lean_unsigned_to_nat(0u);
return v___x_217_;
}
else
{
lean_object* v___x_218_; 
v___x_218_ = lean_unsigned_to_nat(1u);
return v___x_218_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorIdx___boxed(lean_object* v_x_219_){
_start:
{
lean_object* v_res_220_; 
v_res_220_ = lp_clauseflow_ClauseFlow_Concl_ctorIdx(v_x_219_);
lean_dec(v_x_219_);
return v_res_220_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(lean_object* v_t_221_, lean_object* v_k_222_){
_start:
{
if (lean_obj_tag(v_t_221_) == 0)
{
return v_k_222_;
}
else
{
lean_object* v_hs_223_; lean_object* v_ps_224_; lean_object* v___x_225_; 
v_hs_223_ = lean_ctor_get(v_t_221_, 0);
lean_inc(v_hs_223_);
v_ps_224_ = lean_ctor_get(v_t_221_, 1);
lean_inc(v_ps_224_);
lean_dec_ref_known(v_t_221_, 2);
v___x_225_ = lean_apply_2(v_k_222_, v_hs_223_, v_ps_224_);
return v___x_225_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorElim(lean_object* v_motive_226_, lean_object* v_ctorIdx_227_, lean_object* v_t_228_, lean_object* v_h_229_, lean_object* v_k_230_){
_start:
{
lean_object* v___x_231_; 
v___x_231_ = lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(v_t_228_, v_k_230_);
return v___x_231_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ctorElim___boxed(lean_object* v_motive_232_, lean_object* v_ctorIdx_233_, lean_object* v_t_234_, lean_object* v_h_235_, lean_object* v_k_236_){
_start:
{
lean_object* v_res_237_; 
v_res_237_ = lp_clauseflow_ClauseFlow_Concl_ctorElim(v_motive_232_, v_ctorIdx_233_, v_t_234_, v_h_235_, v_k_236_);
lean_dec(v_ctorIdx_233_);
return v_res_237_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_bot_elim___redArg(lean_object* v_t_238_, lean_object* v_bot_239_){
_start:
{
lean_object* v___x_240_; 
v___x_240_ = lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(v_t_238_, v_bot_239_);
return v___x_240_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_bot_elim(lean_object* v_motive_241_, lean_object* v_t_242_, lean_object* v_h_243_, lean_object* v_bot_244_){
_start:
{
lean_object* v___x_245_; 
v___x_245_ = lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(v_t_242_, v_bot_244_);
return v___x_245_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_chk_elim___redArg(lean_object* v_t_246_, lean_object* v_chk_247_){
_start:
{
lean_object* v___x_248_; 
v___x_248_ = lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(v_t_246_, v_chk_247_);
return v___x_248_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_chk_elim(lean_object* v_motive_249_, lean_object* v_t_250_, lean_object* v_h_251_, lean_object* v_chk_252_){
_start:
{
lean_object* v___x_253_; 
v___x_253_ = lp_clauseflow_ClauseFlow_Concl_ctorElim___redArg(v_t_250_, v_chk_252_);
return v___x_253_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq(lean_object* v_x_254_, lean_object* v_x_255_){
_start:
{
if (lean_obj_tag(v_x_254_) == 0)
{
if (lean_obj_tag(v_x_255_) == 0)
{
uint8_t v___x_256_; 
v___x_256_ = 1;
return v___x_256_;
}
else
{
uint8_t v___x_257_; 
lean_dec_ref_known(v_x_255_, 2);
v___x_257_ = 0;
return v___x_257_;
}
}
else
{
lean_object* v_hs_258_; lean_object* v_ps_259_; uint8_t v___x_260_; 
v_hs_258_ = lean_ctor_get(v_x_254_, 0);
lean_inc(v_hs_258_);
v_ps_259_ = lean_ctor_get(v_x_254_, 1);
lean_inc(v_ps_259_);
lean_dec_ref_known(v_x_254_, 2);
v___x_260_ = 0;
if (lean_obj_tag(v_x_255_) == 0)
{
lean_dec(v_ps_259_);
lean_dec(v_hs_258_);
return v___x_260_;
}
else
{
lean_object* v_hs_261_; lean_object* v_ps_262_; lean_object* v___x_263_; uint8_t v___x_264_; 
v_hs_261_ = lean_ctor_get(v_x_255_, 0);
lean_inc(v_hs_261_);
v_ps_262_ = lean_ctor_get(v_x_255_, 1);
lean_inc(v_ps_262_);
lean_dec_ref_known(v_x_255_, 2);
v___x_263_ = lean_alloc_closure((void*)(l_instDecidableEqNat___boxed), 2, 0);
lean_inc_ref(v___x_263_);
v___x_264_ = l_instDecidableEqList___redArg(v___x_263_, v_hs_258_, v_hs_261_);
if (v___x_264_ == 0)
{
lean_dec_ref(v___x_263_);
lean_dec(v_ps_262_);
lean_dec(v_ps_259_);
return v___x_260_;
}
else
{
uint8_t v___x_265_; 
v___x_265_ = l_instDecidableEqList___redArg(v___x_263_, v_ps_259_, v_ps_262_);
if (v___x_265_ == 0)
{
return v___x_260_;
}
else
{
return v___x_265_;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq___boxed(lean_object* v_x_266_, lean_object* v_x_267_){
_start:
{
uint8_t v_res_268_; lean_object* v_r_269_; 
v_res_268_ = lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq(v_x_266_, v_x_267_);
v_r_269_ = lean_box(v_res_268_);
return v_r_269_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqConcl(lean_object* v_x_270_, lean_object* v_x_271_){
_start:
{
uint8_t v___x_272_; 
v___x_272_ = lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq(v_x_270_, v_x_271_);
return v___x_272_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqConcl___boxed(lean_object* v_x_273_, lean_object* v_x_274_){
_start:
{
uint8_t v_res_275_; lean_object* v_r_276_; 
v_res_275_ = lp_clauseflow_ClauseFlow_instDecidableEqConcl(v_x_273_, v_x_274_);
v_r_276_ = lean_box(v_res_275_);
return v_r_276_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0___lam__0(lean_object* v___y_277_){
_start:
{
lean_object* v___x_278_; lean_object* v___x_279_; 
v___x_278_ = l_Nat_reprFast(v___y_277_);
v___x_279_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_279_, 0, v___x_278_);
return v___x_279_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0_spec__1_spec__2(lean_object* v_x_280_, lean_object* v_x_281_, lean_object* v_x_282_){
_start:
{
if (lean_obj_tag(v_x_282_) == 0)
{
lean_dec(v_x_280_);
return v_x_281_;
}
else
{
lean_object* v_head_283_; lean_object* v_tail_284_; lean_object* v___x_286_; uint8_t v_isShared_287_; uint8_t v_isSharedCheck_295_; 
v_head_283_ = lean_ctor_get(v_x_282_, 0);
v_tail_284_ = lean_ctor_get(v_x_282_, 1);
v_isSharedCheck_295_ = !lean_is_exclusive(v_x_282_);
if (v_isSharedCheck_295_ == 0)
{
v___x_286_ = v_x_282_;
v_isShared_287_ = v_isSharedCheck_295_;
goto v_resetjp_285_;
}
else
{
lean_inc(v_tail_284_);
lean_inc(v_head_283_);
lean_dec(v_x_282_);
v___x_286_ = lean_box(0);
v_isShared_287_ = v_isSharedCheck_295_;
goto v_resetjp_285_;
}
v_resetjp_285_:
{
lean_object* v___x_289_; 
lean_inc(v_x_280_);
if (v_isShared_287_ == 0)
{
lean_ctor_set_tag(v___x_286_, 5);
lean_ctor_set(v___x_286_, 1, v_x_280_);
lean_ctor_set(v___x_286_, 0, v_x_281_);
v___x_289_ = v___x_286_;
goto v_reusejp_288_;
}
else
{
lean_object* v_reuseFailAlloc_294_; 
v_reuseFailAlloc_294_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_294_, 0, v_x_281_);
lean_ctor_set(v_reuseFailAlloc_294_, 1, v_x_280_);
v___x_289_ = v_reuseFailAlloc_294_;
goto v_reusejp_288_;
}
v_reusejp_288_:
{
lean_object* v___x_290_; lean_object* v___x_291_; lean_object* v___x_292_; 
v___x_290_ = l_Nat_reprFast(v_head_283_);
v___x_291_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_291_, 0, v___x_290_);
v___x_292_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_292_, 0, v___x_289_);
lean_ctor_set(v___x_292_, 1, v___x_291_);
v_x_281_ = v___x_292_;
v_x_282_ = v_tail_284_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0_spec__1(lean_object* v_x_296_, lean_object* v_x_297_, lean_object* v_x_298_){
_start:
{
if (lean_obj_tag(v_x_298_) == 0)
{
lean_dec(v_x_296_);
return v_x_297_;
}
else
{
lean_object* v_head_299_; lean_object* v_tail_300_; lean_object* v___x_302_; uint8_t v_isShared_303_; uint8_t v_isSharedCheck_311_; 
v_head_299_ = lean_ctor_get(v_x_298_, 0);
v_tail_300_ = lean_ctor_get(v_x_298_, 1);
v_isSharedCheck_311_ = !lean_is_exclusive(v_x_298_);
if (v_isSharedCheck_311_ == 0)
{
v___x_302_ = v_x_298_;
v_isShared_303_ = v_isSharedCheck_311_;
goto v_resetjp_301_;
}
else
{
lean_inc(v_tail_300_);
lean_inc(v_head_299_);
lean_dec(v_x_298_);
v___x_302_ = lean_box(0);
v_isShared_303_ = v_isSharedCheck_311_;
goto v_resetjp_301_;
}
v_resetjp_301_:
{
lean_object* v___x_305_; 
lean_inc(v_x_296_);
if (v_isShared_303_ == 0)
{
lean_ctor_set_tag(v___x_302_, 5);
lean_ctor_set(v___x_302_, 1, v_x_296_);
lean_ctor_set(v___x_302_, 0, v_x_297_);
v___x_305_ = v___x_302_;
goto v_reusejp_304_;
}
else
{
lean_object* v_reuseFailAlloc_310_; 
v_reuseFailAlloc_310_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_310_, 0, v_x_297_);
lean_ctor_set(v_reuseFailAlloc_310_, 1, v_x_296_);
v___x_305_ = v_reuseFailAlloc_310_;
goto v_reusejp_304_;
}
v_reusejp_304_:
{
lean_object* v___x_306_; lean_object* v___x_307_; lean_object* v___x_308_; lean_object* v___x_309_; 
v___x_306_ = l_Nat_reprFast(v_head_299_);
v___x_307_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_307_, 0, v___x_306_);
v___x_308_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_308_, 0, v___x_305_);
lean_ctor_set(v___x_308_, 1, v___x_307_);
v___x_309_ = lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0_spec__1_spec__2(v_x_296_, v___x_308_, v_tail_300_);
return v___x_309_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0(lean_object* v_x_312_, lean_object* v_x_313_){
_start:
{
if (lean_obj_tag(v_x_312_) == 0)
{
lean_object* v___x_314_; 
lean_dec(v_x_313_);
v___x_314_ = lean_box(0);
return v___x_314_;
}
else
{
lean_object* v_tail_315_; 
v_tail_315_ = lean_ctor_get(v_x_312_, 1);
if (lean_obj_tag(v_tail_315_) == 0)
{
lean_object* v_head_316_; lean_object* v___x_317_; 
lean_dec(v_x_313_);
v_head_316_ = lean_ctor_get(v_x_312_, 0);
lean_inc(v_head_316_);
lean_dec_ref_known(v_x_312_, 2);
v___x_317_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0___lam__0(v_head_316_);
return v___x_317_;
}
else
{
lean_object* v_head_318_; lean_object* v___x_319_; lean_object* v___x_320_; 
lean_inc(v_tail_315_);
v_head_318_ = lean_ctor_get(v_x_312_, 0);
lean_inc(v_head_318_);
lean_dec_ref_known(v_x_312_, 2);
v___x_319_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0___lam__0(v_head_318_);
v___x_320_ = lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0_spec__1(v_x_313_, v___x_319_, v_tail_315_);
return v___x_320_;
}
}
}
}
static lean_object* _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__7(void){
_start:
{
lean_object* v___x_332_; lean_object* v___x_333_; 
v___x_332_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__2));
v___x_333_ = lean_string_length(v___x_332_);
return v___x_333_;
}
}
static lean_object* _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8(void){
_start:
{
lean_object* v___x_334_; lean_object* v___x_335_; 
v___x_334_ = lean_obj_once(&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__7, &lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__7_once, _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__7);
v___x_335_ = lean_nat_to_int(v___x_334_);
return v___x_335_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(lean_object* v_a_340_){
_start:
{
if (lean_obj_tag(v_a_340_) == 0)
{
lean_object* v___x_341_; 
v___x_341_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1));
return v___x_341_;
}
else
{
lean_object* v___x_342_; lean_object* v___x_343_; lean_object* v___x_344_; lean_object* v___x_345_; lean_object* v___x_346_; lean_object* v___x_347_; lean_object* v___x_348_; lean_object* v___x_349_; lean_object* v___x_350_; 
v___x_342_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_343_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0_spec__0(v_a_340_, v___x_342_);
v___x_344_ = lean_obj_once(&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8, &lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8_once, _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8);
v___x_345_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9));
v___x_346_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_346_, 0, v___x_345_);
lean_ctor_set(v___x_346_, 1, v___x_343_);
v___x_347_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10));
v___x_348_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_348_, 0, v___x_346_);
lean_ctor_set(v___x_348_, 1, v___x_347_);
v___x_349_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_349_, 0, v___x_344_);
lean_ctor_set(v___x_349_, 1, v___x_348_);
v___x_350_ = l_Std_Format_fill(v___x_349_);
return v___x_350_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr(lean_object* v_x_360_, lean_object* v_prec_361_){
_start:
{
lean_object* v___y_363_; 
if (lean_obj_tag(v_x_360_) == 0)
{
lean_object* v___x_369_; uint8_t v___x_370_; 
v___x_369_ = lean_unsigned_to_nat(1024u);
v___x_370_ = lean_nat_dec_le(v___x_369_, v_prec_361_);
if (v___x_370_ == 0)
{
lean_object* v___x_371_; 
v___x_371_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_363_ = v___x_371_;
goto v___jp_362_;
}
else
{
lean_object* v___x_372_; 
v___x_372_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_363_ = v___x_372_;
goto v___jp_362_;
}
}
else
{
lean_object* v_hs_373_; lean_object* v_ps_374_; lean_object* v___x_376_; uint8_t v_isShared_377_; uint8_t v_isSharedCheck_397_; 
v_hs_373_ = lean_ctor_get(v_x_360_, 0);
v_ps_374_ = lean_ctor_get(v_x_360_, 1);
v_isSharedCheck_397_ = !lean_is_exclusive(v_x_360_);
if (v_isSharedCheck_397_ == 0)
{
v___x_376_ = v_x_360_;
v_isShared_377_ = v_isSharedCheck_397_;
goto v_resetjp_375_;
}
else
{
lean_inc(v_ps_374_);
lean_inc(v_hs_373_);
lean_dec(v_x_360_);
v___x_376_ = lean_box(0);
v_isShared_377_ = v_isSharedCheck_397_;
goto v_resetjp_375_;
}
v_resetjp_375_:
{
lean_object* v___y_379_; lean_object* v___x_393_; uint8_t v___x_394_; 
v___x_393_ = lean_unsigned_to_nat(1024u);
v___x_394_ = lean_nat_dec_le(v___x_393_, v_prec_361_);
if (v___x_394_ == 0)
{
lean_object* v___x_395_; 
v___x_395_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_379_ = v___x_395_;
goto v___jp_378_;
}
else
{
lean_object* v___x_396_; 
v___x_396_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_379_ = v___x_396_;
goto v___jp_378_;
}
v___jp_378_:
{
lean_object* v___x_380_; lean_object* v___x_381_; lean_object* v___x_382_; lean_object* v___x_384_; 
v___x_380_ = lean_box(1);
v___x_381_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__4));
v___x_382_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_hs_373_);
if (v_isShared_377_ == 0)
{
lean_ctor_set_tag(v___x_376_, 5);
lean_ctor_set(v___x_376_, 1, v___x_382_);
lean_ctor_set(v___x_376_, 0, v___x_381_);
v___x_384_ = v___x_376_;
goto v_reusejp_383_;
}
else
{
lean_object* v_reuseFailAlloc_392_; 
v_reuseFailAlloc_392_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_392_, 0, v___x_381_);
lean_ctor_set(v_reuseFailAlloc_392_, 1, v___x_382_);
v___x_384_ = v_reuseFailAlloc_392_;
goto v_reusejp_383_;
}
v_reusejp_383_:
{
lean_object* v___x_385_; lean_object* v___x_386_; lean_object* v___x_387_; lean_object* v___x_388_; uint8_t v___x_389_; lean_object* v___x_390_; lean_object* v___x_391_; 
v___x_385_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_385_, 0, v___x_384_);
lean_ctor_set(v___x_385_, 1, v___x_380_);
v___x_386_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_ps_374_);
v___x_387_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_387_, 0, v___x_385_);
lean_ctor_set(v___x_387_, 1, v___x_386_);
lean_inc(v___y_379_);
v___x_388_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_388_, 0, v___y_379_);
lean_ctor_set(v___x_388_, 1, v___x_387_);
v___x_389_ = 0;
v___x_390_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_390_, 0, v___x_388_);
lean_ctor_set_uint8(v___x_390_, sizeof(void*)*1, v___x_389_);
v___x_391_ = l_Repr_addAppParen(v___x_390_, v_prec_361_);
return v___x_391_;
}
}
}
}
v___jp_362_:
{
lean_object* v___x_364_; lean_object* v___x_365_; uint8_t v___x_366_; lean_object* v___x_367_; lean_object* v___x_368_; 
v___x_364_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprConcl_repr___closed__1));
lean_inc(v___y_363_);
v___x_365_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_365_, 0, v___y_363_);
lean_ctor_set(v___x_365_, 1, v___x_364_);
v___x_366_ = 0;
v___x_367_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_367_, 0, v___x_365_);
lean_ctor_set_uint8(v___x_367_, sizeof(void*)*1, v___x_366_);
v___x_368_ = l_Repr_addAppParen(v___x_367_, v_prec_361_);
return v___x_368_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprConcl_repr___boxed(lean_object* v_x_398_, lean_object* v_prec_399_){
_start:
{
lean_object* v_res_400_; 
v_res_400_ = lp_clauseflow_ClauseFlow_instReprConcl_repr(v_x_398_, v_prec_399_);
lean_dec(v_prec_399_);
return v_res_400_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0(lean_object* v_a_401_, lean_object* v_n_402_){
_start:
{
lean_object* v___x_403_; 
v___x_403_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_a_401_);
return v___x_403_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___boxed(lean_object* v_a_404_, lean_object* v_n_405_){
_start:
{
lean_object* v_res_406_; 
v_res_406_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0(v_a_404_, v_n_405_);
lean_dec(v_n_405_);
return v_res_406_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___lam__0(lean_object* v_a_409_, lean_object* v_b_410_){
_start:
{
lean_object* v___x_411_; lean_object* v___x_412_; uint8_t v___x_413_; 
v___x_411_ = lean_alloc_closure((void*)(l_instDecidableEqNat___boxed), 2, 0);
v___x_412_ = lean_alloc_closure((void*)(lp_clauseflow_ClauseFlow_instDecidableEqTest___boxed), 2, 0);
v___x_413_ = l_instDecidableEqProd___redArg(v___x_411_, v___x_412_, v_a_409_, v_b_410_);
return v___x_413_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___lam__0___boxed(lean_object* v_a_414_, lean_object* v_b_415_){
_start:
{
uint8_t v_res_416_; lean_object* v_r_417_; 
v_res_416_ = lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___lam__0(v_a_414_, v_b_415_);
v_r_417_ = lean_box(v_res_416_);
return v_r_417_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq(lean_object* v_x_419_, lean_object* v_x_420_){
_start:
{
lean_object* v_gs_421_; lean_object* v_c_422_; lean_object* v_gs_423_; lean_object* v_c_424_; lean_object* v___f_425_; uint8_t v___x_426_; 
v_gs_421_ = lean_ctor_get(v_x_419_, 0);
lean_inc(v_gs_421_);
v_c_422_ = lean_ctor_get(v_x_419_, 1);
lean_inc(v_c_422_);
lean_dec_ref(v_x_419_);
v_gs_423_ = lean_ctor_get(v_x_420_, 0);
lean_inc(v_gs_423_);
v_c_424_ = lean_ctor_get(v_x_420_, 1);
lean_inc(v_c_424_);
lean_dec_ref(v_x_420_);
v___f_425_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___closed__0));
v___x_426_ = l_instDecidableEqList___redArg(v___f_425_, v_gs_421_, v_gs_423_);
if (v___x_426_ == 0)
{
lean_dec(v_c_424_);
lean_dec(v_c_422_);
return v___x_426_;
}
else
{
uint8_t v___x_427_; 
v___x_427_ = lp_clauseflow_ClauseFlow_instDecidableEqConcl_decEq(v_c_422_, v_c_424_);
return v___x_427_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq___boxed(lean_object* v_x_428_, lean_object* v_x_429_){
_start:
{
uint8_t v_res_430_; lean_object* v_r_431_; 
v_res_430_ = lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq(v_x_428_, v_x_429_);
v_r_431_ = lean_box(v_res_430_);
return v_r_431_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqClause(lean_object* v_x_432_, lean_object* v_x_433_){
_start:
{
uint8_t v___x_434_; 
v___x_434_ = lp_clauseflow_ClauseFlow_instDecidableEqClause_decEq(v_x_432_, v_x_433_);
return v___x_434_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqClause___boxed(lean_object* v_x_435_, lean_object* v_x_436_){
_start:
{
uint8_t v_res_437_; lean_object* v_r_438_; 
v_res_437_ = lp_clauseflow_ClauseFlow_instDecidableEqClause(v_x_435_, v_x_436_);
v_r_438_ = lean_box(v_res_437_);
return v_r_438_;
}
}
static lean_object* _init_lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__2(void){
_start:
{
lean_object* v___x_441_; lean_object* v___x_442_; 
v___x_441_ = ((lean_object*)(lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__0));
v___x_442_ = lean_string_length(v___x_441_);
return v___x_442_;
}
}
static lean_object* _init_lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3(void){
_start:
{
lean_object* v___x_443_; lean_object* v___x_444_; 
v___x_443_ = lean_obj_once(&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__2, &lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__2_once, _init_lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__2);
v___x_444_ = lean_nat_to_int(v___x_443_);
return v___x_444_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(lean_object* v_x_449_){
_start:
{
lean_object* v_fst_450_; lean_object* v_snd_451_; lean_object* v___x_453_; uint8_t v_isShared_454_; uint8_t v_isSharedCheck_475_; 
v_fst_450_ = lean_ctor_get(v_x_449_, 0);
v_snd_451_ = lean_ctor_get(v_x_449_, 1);
v_isSharedCheck_475_ = !lean_is_exclusive(v_x_449_);
if (v_isSharedCheck_475_ == 0)
{
v___x_453_ = v_x_449_;
v_isShared_454_ = v_isSharedCheck_475_;
goto v_resetjp_452_;
}
else
{
lean_inc(v_snd_451_);
lean_inc(v_fst_450_);
lean_dec(v_x_449_);
v___x_453_ = lean_box(0);
v_isShared_454_ = v_isSharedCheck_475_;
goto v_resetjp_452_;
}
v_resetjp_452_:
{
lean_object* v___x_455_; lean_object* v___x_456_; lean_object* v___x_457_; lean_object* v___x_459_; 
v___x_455_ = l_Nat_reprFast(v_fst_450_);
v___x_456_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_456_, 0, v___x_455_);
v___x_457_ = lean_box(0);
if (v_isShared_454_ == 0)
{
lean_ctor_set_tag(v___x_453_, 1);
lean_ctor_set(v___x_453_, 1, v___x_457_);
lean_ctor_set(v___x_453_, 0, v___x_456_);
v___x_459_ = v___x_453_;
goto v_reusejp_458_;
}
else
{
lean_object* v_reuseFailAlloc_474_; 
v_reuseFailAlloc_474_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_474_, 0, v___x_456_);
lean_ctor_set(v_reuseFailAlloc_474_, 1, v___x_457_);
v___x_459_ = v_reuseFailAlloc_474_;
goto v_reusejp_458_;
}
v_reusejp_458_:
{
lean_object* v___x_460_; lean_object* v___x_461_; lean_object* v___x_462_; lean_object* v___x_463_; lean_object* v___x_464_; lean_object* v___x_465_; lean_object* v___x_466_; lean_object* v___x_467_; lean_object* v___x_468_; lean_object* v___x_469_; lean_object* v___x_470_; lean_object* v___x_471_; uint8_t v___x_472_; lean_object* v___x_473_; 
v___x_460_ = lean_unsigned_to_nat(0u);
v___x_461_ = lp_clauseflow_ClauseFlow_instReprTest_repr(v_snd_451_, v___x_460_);
v___x_462_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_462_, 0, v___x_461_);
lean_ctor_set(v___x_462_, 1, v___x_459_);
v___x_463_ = l_List_reverse___redArg(v___x_462_);
v___x_464_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_465_ = l_Std_Format_joinSep___at___00Lean_Syntax_formatStxAux_spec__2(v___x_463_, v___x_464_);
v___x_466_ = lean_obj_once(&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3, &lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3_once, _init_lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3);
v___x_467_ = ((lean_object*)(lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__4));
v___x_468_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_468_, 0, v___x_467_);
lean_ctor_set(v___x_468_, 1, v___x_465_);
v___x_469_ = ((lean_object*)(lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__5));
v___x_470_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_470_, 0, v___x_468_);
lean_ctor_set(v___x_470_, 1, v___x_469_);
v___x_471_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_471_, 0, v___x_466_);
lean_ctor_set(v___x_471_, 1, v___x_470_);
v___x_472_ = 0;
v___x_473_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_473_, 0, v___x_471_);
lean_ctor_set_uint8(v___x_473_, sizeof(void*)*1, v___x_472_);
return v___x_473_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1_spec__2_spec__3(lean_object* v_x_476_, lean_object* v_x_477_, lean_object* v_x_478_){
_start:
{
if (lean_obj_tag(v_x_478_) == 0)
{
lean_dec(v_x_476_);
return v_x_477_;
}
else
{
lean_object* v_head_479_; lean_object* v_tail_480_; lean_object* v___x_482_; uint8_t v_isShared_483_; uint8_t v_isSharedCheck_490_; 
v_head_479_ = lean_ctor_get(v_x_478_, 0);
v_tail_480_ = lean_ctor_get(v_x_478_, 1);
v_isSharedCheck_490_ = !lean_is_exclusive(v_x_478_);
if (v_isSharedCheck_490_ == 0)
{
v___x_482_ = v_x_478_;
v_isShared_483_ = v_isSharedCheck_490_;
goto v_resetjp_481_;
}
else
{
lean_inc(v_tail_480_);
lean_inc(v_head_479_);
lean_dec(v_x_478_);
v___x_482_ = lean_box(0);
v_isShared_483_ = v_isSharedCheck_490_;
goto v_resetjp_481_;
}
v_resetjp_481_:
{
lean_object* v___x_485_; 
lean_inc(v_x_476_);
if (v_isShared_483_ == 0)
{
lean_ctor_set_tag(v___x_482_, 5);
lean_ctor_set(v___x_482_, 1, v_x_476_);
lean_ctor_set(v___x_482_, 0, v_x_477_);
v___x_485_ = v___x_482_;
goto v_reusejp_484_;
}
else
{
lean_object* v_reuseFailAlloc_489_; 
v_reuseFailAlloc_489_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_489_, 0, v_x_477_);
lean_ctor_set(v_reuseFailAlloc_489_, 1, v_x_476_);
v___x_485_ = v_reuseFailAlloc_489_;
goto v_reusejp_484_;
}
v_reusejp_484_:
{
lean_object* v___x_486_; lean_object* v___x_487_; 
v___x_486_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(v_head_479_);
v___x_487_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_487_, 0, v___x_485_);
lean_ctor_set(v___x_487_, 1, v___x_486_);
v_x_477_ = v___x_487_;
v_x_478_ = v_tail_480_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1_spec__2(lean_object* v_x_491_, lean_object* v_x_492_, lean_object* v_x_493_){
_start:
{
if (lean_obj_tag(v_x_493_) == 0)
{
lean_dec(v_x_491_);
return v_x_492_;
}
else
{
lean_object* v_head_494_; lean_object* v_tail_495_; lean_object* v___x_497_; uint8_t v_isShared_498_; uint8_t v_isSharedCheck_505_; 
v_head_494_ = lean_ctor_get(v_x_493_, 0);
v_tail_495_ = lean_ctor_get(v_x_493_, 1);
v_isSharedCheck_505_ = !lean_is_exclusive(v_x_493_);
if (v_isSharedCheck_505_ == 0)
{
v___x_497_ = v_x_493_;
v_isShared_498_ = v_isSharedCheck_505_;
goto v_resetjp_496_;
}
else
{
lean_inc(v_tail_495_);
lean_inc(v_head_494_);
lean_dec(v_x_493_);
v___x_497_ = lean_box(0);
v_isShared_498_ = v_isSharedCheck_505_;
goto v_resetjp_496_;
}
v_resetjp_496_:
{
lean_object* v___x_500_; 
lean_inc(v_x_491_);
if (v_isShared_498_ == 0)
{
lean_ctor_set_tag(v___x_497_, 5);
lean_ctor_set(v___x_497_, 1, v_x_491_);
lean_ctor_set(v___x_497_, 0, v_x_492_);
v___x_500_ = v___x_497_;
goto v_reusejp_499_;
}
else
{
lean_object* v_reuseFailAlloc_504_; 
v_reuseFailAlloc_504_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_504_, 0, v_x_492_);
lean_ctor_set(v_reuseFailAlloc_504_, 1, v_x_491_);
v___x_500_ = v_reuseFailAlloc_504_;
goto v_reusejp_499_;
}
v_reusejp_499_:
{
lean_object* v___x_501_; lean_object* v___x_502_; lean_object* v___x_503_; 
v___x_501_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(v_head_494_);
v___x_502_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_502_, 0, v___x_500_);
lean_ctor_set(v___x_502_, 1, v___x_501_);
v___x_503_ = lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1_spec__2_spec__3(v_x_491_, v___x_502_, v_tail_495_);
return v___x_503_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1(lean_object* v_x_506_, lean_object* v_x_507_){
_start:
{
if (lean_obj_tag(v_x_506_) == 0)
{
lean_object* v___x_508_; 
lean_dec(v_x_507_);
v___x_508_ = lean_box(0);
return v___x_508_;
}
else
{
lean_object* v_tail_509_; 
v_tail_509_ = lean_ctor_get(v_x_506_, 1);
if (lean_obj_tag(v_tail_509_) == 0)
{
lean_object* v_head_510_; lean_object* v___x_511_; 
lean_dec(v_x_507_);
v_head_510_ = lean_ctor_get(v_x_506_, 0);
lean_inc(v_head_510_);
lean_dec_ref_known(v_x_506_, 2);
v___x_511_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(v_head_510_);
return v___x_511_;
}
else
{
lean_object* v_head_512_; lean_object* v___x_513_; lean_object* v___x_514_; 
lean_inc(v_tail_509_);
v_head_512_ = lean_ctor_get(v_x_506_, 0);
lean_inc(v_head_512_);
lean_dec_ref_known(v_x_506_, 2);
v___x_513_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(v_head_512_);
v___x_514_ = lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1_spec__2(v_x_507_, v___x_513_, v_tail_509_);
return v___x_514_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___redArg(lean_object* v_a_515_){
_start:
{
if (lean_obj_tag(v_a_515_) == 0)
{
lean_object* v___x_516_; 
v___x_516_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1));
return v___x_516_;
}
else
{
lean_object* v___x_517_; lean_object* v___x_518_; lean_object* v___x_519_; lean_object* v___x_520_; lean_object* v___x_521_; lean_object* v___x_522_; lean_object* v___x_523_; lean_object* v___x_524_; uint8_t v___x_525_; lean_object* v___x_526_; 
v___x_517_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_518_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__1(v_a_515_, v___x_517_);
v___x_519_ = lean_obj_once(&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8, &lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8_once, _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8);
v___x_520_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9));
v___x_521_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_521_, 0, v___x_520_);
lean_ctor_set(v___x_521_, 1, v___x_518_);
v___x_522_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10));
v___x_523_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_523_, 0, v___x_521_);
lean_ctor_set(v___x_523_, 1, v___x_522_);
v___x_524_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_524_, 0, v___x_519_);
lean_ctor_set(v___x_524_, 1, v___x_523_);
v___x_525_ = 0;
v___x_526_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_526_, 0, v___x_524_);
lean_ctor_set_uint8(v___x_526_, sizeof(void*)*1, v___x_525_);
return v___x_526_;
}
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__7(void){
_start:
{
lean_object* v___x_540_; lean_object* v___x_541_; 
v___x_540_ = lean_unsigned_to_nat(6u);
v___x_541_ = lean_nat_to_int(v___x_540_);
return v___x_541_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__10(void){
_start:
{
lean_object* v___x_545_; lean_object* v___x_546_; 
v___x_545_ = lean_unsigned_to_nat(5u);
v___x_546_ = lean_nat_to_int(v___x_545_);
return v___x_546_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__12(void){
_start:
{
lean_object* v___x_548_; lean_object* v___x_549_; 
v___x_548_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__0));
v___x_549_ = lean_string_length(v___x_548_);
return v___x_549_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13(void){
_start:
{
lean_object* v___x_550_; lean_object* v___x_551_; 
v___x_550_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__12, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__12_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__12);
v___x_551_ = lean_nat_to_int(v___x_550_);
return v___x_551_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(lean_object* v_x_556_){
_start:
{
lean_object* v_gs_557_; lean_object* v_c_558_; lean_object* v___x_560_; uint8_t v_isShared_561_; uint8_t v_isSharedCheck_592_; 
v_gs_557_ = lean_ctor_get(v_x_556_, 0);
v_c_558_ = lean_ctor_get(v_x_556_, 1);
v_isSharedCheck_592_ = !lean_is_exclusive(v_x_556_);
if (v_isSharedCheck_592_ == 0)
{
v___x_560_ = v_x_556_;
v_isShared_561_ = v_isSharedCheck_592_;
goto v_resetjp_559_;
}
else
{
lean_inc(v_c_558_);
lean_inc(v_gs_557_);
lean_dec(v_x_556_);
v___x_560_ = lean_box(0);
v_isShared_561_ = v_isSharedCheck_592_;
goto v_resetjp_559_;
}
v_resetjp_559_:
{
lean_object* v___x_562_; lean_object* v___x_563_; lean_object* v___x_564_; lean_object* v___x_565_; lean_object* v___x_566_; lean_object* v___x_568_; 
v___x_562_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5));
v___x_563_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__6));
v___x_564_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__7, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__7_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__7);
v___x_565_ = lean_unsigned_to_nat(0u);
v___x_566_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___redArg(v_gs_557_);
if (v_isShared_561_ == 0)
{
lean_ctor_set_tag(v___x_560_, 4);
lean_ctor_set(v___x_560_, 1, v___x_566_);
lean_ctor_set(v___x_560_, 0, v___x_564_);
v___x_568_ = v___x_560_;
goto v_reusejp_567_;
}
else
{
lean_object* v_reuseFailAlloc_591_; 
v_reuseFailAlloc_591_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v_reuseFailAlloc_591_, 0, v___x_564_);
lean_ctor_set(v_reuseFailAlloc_591_, 1, v___x_566_);
v___x_568_ = v_reuseFailAlloc_591_;
goto v_reusejp_567_;
}
v_reusejp_567_:
{
uint8_t v___x_569_; lean_object* v___x_570_; lean_object* v___x_571_; lean_object* v___x_572_; lean_object* v___x_573_; lean_object* v___x_574_; lean_object* v___x_575_; lean_object* v___x_576_; lean_object* v___x_577_; lean_object* v___x_578_; lean_object* v___x_579_; lean_object* v___x_580_; lean_object* v___x_581_; lean_object* v___x_582_; lean_object* v___x_583_; lean_object* v___x_584_; lean_object* v___x_585_; lean_object* v___x_586_; lean_object* v___x_587_; lean_object* v___x_588_; lean_object* v___x_589_; lean_object* v___x_590_; 
v___x_569_ = 0;
v___x_570_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_570_, 0, v___x_568_);
lean_ctor_set_uint8(v___x_570_, sizeof(void*)*1, v___x_569_);
v___x_571_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_571_, 0, v___x_563_);
lean_ctor_set(v___x_571_, 1, v___x_570_);
v___x_572_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4));
v___x_573_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_573_, 0, v___x_571_);
lean_ctor_set(v___x_573_, 1, v___x_572_);
v___x_574_ = lean_box(1);
v___x_575_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_575_, 0, v___x_573_);
lean_ctor_set(v___x_575_, 1, v___x_574_);
v___x_576_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__9));
v___x_577_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_577_, 0, v___x_575_);
lean_ctor_set(v___x_577_, 1, v___x_576_);
v___x_578_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_578_, 0, v___x_577_);
lean_ctor_set(v___x_578_, 1, v___x_562_);
v___x_579_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__10, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__10_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__10);
v___x_580_ = lp_clauseflow_ClauseFlow_instReprConcl_repr(v_c_558_, v___x_565_);
v___x_581_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_581_, 0, v___x_579_);
lean_ctor_set(v___x_581_, 1, v___x_580_);
v___x_582_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_582_, 0, v___x_581_);
lean_ctor_set_uint8(v___x_582_, sizeof(void*)*1, v___x_569_);
v___x_583_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_583_, 0, v___x_578_);
lean_ctor_set(v___x_583_, 1, v___x_582_);
v___x_584_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13);
v___x_585_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14));
v___x_586_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_586_, 0, v___x_585_);
lean_ctor_set(v___x_586_, 1, v___x_583_);
v___x_587_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15));
v___x_588_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_588_, 0, v___x_586_);
lean_ctor_set(v___x_588_, 1, v___x_587_);
v___x_589_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_589_, 0, v___x_584_);
lean_ctor_set(v___x_589_, 1, v___x_588_);
v___x_590_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_590_, 0, v___x_589_);
lean_ctor_set_uint8(v___x_590_, sizeof(void*)*1, v___x_569_);
return v___x_590_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr(lean_object* v_x_593_, lean_object* v_prec_594_){
_start:
{
lean_object* v___x_595_; 
v___x_595_ = lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(v_x_593_);
return v___x_595_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprClause_repr___boxed(lean_object* v_x_596_, lean_object* v_prec_597_){
_start:
{
lean_object* v_res_598_; 
v_res_598_ = lp_clauseflow_ClauseFlow_instReprClause_repr(v_x_596_, v_prec_597_);
lean_dec(v_prec_597_);
return v_res_598_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0(lean_object* v_a_599_, lean_object* v_n_600_){
_start:
{
lean_object* v___x_601_; 
v___x_601_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___redArg(v_a_599_);
return v___x_601_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___boxed(lean_object* v_a_602_, lean_object* v_n_603_){
_start:
{
lean_object* v_res_604_; 
v_res_604_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0(v_a_602_, v_n_603_);
lean_dec(v_n_603_);
return v_res_604_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0(lean_object* v_x_605_, lean_object* v_x_606_){
_start:
{
lean_object* v___x_607_; 
v___x_607_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg(v_x_605_);
return v___x_607_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___boxed(lean_object* v_x_608_, lean_object* v_x_609_){
_start:
{
lean_object* v_res_610_; 
v_res_610_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0(v_x_608_, v_x_609_);
lean_dec(v_x_609_);
return v_res_610_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_Concl_isBot(lean_object* v_x_613_){
_start:
{
if (lean_obj_tag(v_x_613_) == 0)
{
uint8_t v___x_614_; 
v___x_614_ = 1;
return v___x_614_;
}
else
{
uint8_t v___x_615_; 
v___x_615_ = 0;
return v___x_615_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_isBot___boxed(lean_object* v_x_616_){
_start:
{
uint8_t v_res_617_; lean_object* v_r_618_; 
v_res_617_ = lp_clauseflow_ClauseFlow_Concl_isBot(v_x_616_);
lean_dec(v_x_616_);
v_r_618_ = lean_box(v_res_617_);
return v_r_618_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_hs(lean_object* v_x_619_){
_start:
{
if (lean_obj_tag(v_x_619_) == 0)
{
lean_object* v___x_620_; 
v___x_620_ = lean_box(0);
return v___x_620_;
}
else
{
lean_object* v_hs_621_; 
v_hs_621_ = lean_ctor_get(v_x_619_, 0);
lean_inc(v_hs_621_);
return v_hs_621_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_hs___boxed(lean_object* v_x_622_){
_start:
{
lean_object* v_res_623_; 
v_res_623_ = lp_clauseflow_ClauseFlow_Concl_hs(v_x_622_);
lean_dec(v_x_622_);
return v_res_623_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ps(lean_object* v_x_624_){
_start:
{
if (lean_obj_tag(v_x_624_) == 0)
{
lean_object* v___x_625_; 
v___x_625_ = lean_box(0);
return v___x_625_;
}
else
{
lean_object* v_ps_626_; 
v_ps_626_ = lean_ctor_get(v_x_624_, 1);
lean_inc(v_ps_626_);
return v_ps_626_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_ps___boxed(lean_object* v_x_627_){
_start:
{
lean_object* v_res_628_; 
v_res_628_ = lp_clauseflow_ClauseFlow_Concl_ps(v_x_627_);
lean_dec(v_x_627_);
return v_res_628_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorIdx(lean_object* v_x_629_){
_start:
{
switch(lean_obj_tag(v_x_629_))
{
case 0:
{
lean_object* v___x_630_; 
v___x_630_ = lean_unsigned_to_nat(0u);
return v___x_630_;
}
case 1:
{
lean_object* v___x_631_; 
v___x_631_ = lean_unsigned_to_nat(1u);
return v___x_631_;
}
case 2:
{
lean_object* v___x_632_; 
v___x_632_ = lean_unsigned_to_nat(2u);
return v___x_632_;
}
case 3:
{
lean_object* v___x_633_; 
v___x_633_ = lean_unsigned_to_nat(3u);
return v___x_633_;
}
case 4:
{
lean_object* v___x_634_; 
v___x_634_ = lean_unsigned_to_nat(4u);
return v___x_634_;
}
default: 
{
lean_object* v___x_635_; 
v___x_635_ = lean_unsigned_to_nat(5u);
return v___x_635_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorIdx___boxed(lean_object* v_x_636_){
_start:
{
lean_object* v_res_637_; 
v_res_637_ = lp_clauseflow_ClauseFlow_Stmt_ctorIdx(v_x_636_);
lean_dec_ref(v_x_636_);
return v_res_637_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(lean_object* v_t_638_, lean_object* v_k_639_){
_start:
{
switch(lean_obj_tag(v_t_638_))
{
case 2:
{
lean_object* v_l_640_; lean_object* v_d_641_; lean_object* v___x_642_; 
v_l_640_ = lean_ctor_get(v_t_638_, 0);
lean_inc(v_l_640_);
v_d_641_ = lean_ctor_get(v_t_638_, 1);
lean_inc(v_d_641_);
lean_dec_ref_known(v_t_638_, 2);
v___x_642_ = lean_apply_2(v_k_639_, v_l_640_, v_d_641_);
return v___x_642_;
}
case 3:
{
lean_object* v_l_643_; lean_object* v_m_644_; lean_object* v___x_645_; 
v_l_643_ = lean_ctor_get(v_t_638_, 0);
lean_inc(v_l_643_);
v_m_644_ = lean_ctor_get(v_t_638_, 1);
lean_inc(v_m_644_);
lean_dec_ref_known(v_t_638_, 2);
v___x_645_ = lean_apply_2(v_k_639_, v_l_643_, v_m_644_);
return v___x_645_;
}
case 4:
{
lean_object* v_l_646_; lean_object* v_m_647_; lean_object* v_n_648_; lean_object* v___x_649_; 
v_l_646_ = lean_ctor_get(v_t_638_, 0);
lean_inc(v_l_646_);
v_m_647_ = lean_ctor_get(v_t_638_, 1);
lean_inc(v_m_647_);
v_n_648_ = lean_ctor_get(v_t_638_, 2);
lean_inc(v_n_648_);
lean_dec_ref_known(v_t_638_, 3);
v___x_649_ = lean_apply_3(v_k_639_, v_l_646_, v_m_647_, v_n_648_);
return v___x_649_;
}
case 5:
{
lean_object* v_l_650_; lean_object* v_m_651_; lean_object* v_n_652_; lean_object* v___x_653_; 
v_l_650_ = lean_ctor_get(v_t_638_, 0);
lean_inc(v_l_650_);
v_m_651_ = lean_ctor_get(v_t_638_, 1);
lean_inc(v_m_651_);
v_n_652_ = lean_ctor_get(v_t_638_, 2);
lean_inc(v_n_652_);
lean_dec_ref_known(v_t_638_, 3);
v___x_653_ = lean_apply_3(v_k_639_, v_l_650_, v_m_651_, v_n_652_);
return v___x_653_;
}
default: 
{
lean_object* v_l_654_; lean_object* v___x_655_; 
v_l_654_ = lean_ctor_get(v_t_638_, 0);
lean_inc(v_l_654_);
lean_dec_ref(v_t_638_);
v___x_655_ = lean_apply_1(v_k_639_, v_l_654_);
return v___x_655_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorElim(lean_object* v_motive_656_, lean_object* v_ctorIdx_657_, lean_object* v_t_658_, lean_object* v_h_659_, lean_object* v_k_660_){
_start:
{
lean_object* v___x_661_; 
v___x_661_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_658_, v_k_660_);
return v___x_661_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_ctorElim___boxed(lean_object* v_motive_662_, lean_object* v_ctorIdx_663_, lean_object* v_t_664_, lean_object* v_h_665_, lean_object* v_k_666_){
_start:
{
lean_object* v_res_667_; 
v_res_667_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim(v_motive_662_, v_ctorIdx_663_, v_t_664_, v_h_665_, v_k_666_);
lean_dec(v_ctorIdx_663_);
return v_res_667_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_havoc_elim___redArg(lean_object* v_t_668_, lean_object* v_havoc_669_){
_start:
{
lean_object* v___x_670_; 
v___x_670_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_668_, v_havoc_669_);
return v___x_670_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_havoc_elim(lean_object* v_motive_671_, lean_object* v_t_672_, lean_object* v_h_673_, lean_object* v_havoc_674_){
_start:
{
lean_object* v___x_675_; 
v___x_675_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_672_, v_havoc_674_);
return v___x_675_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_write_elim___redArg(lean_object* v_t_676_, lean_object* v_write_677_){
_start:
{
lean_object* v___x_678_; 
v___x_678_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_676_, v_write_677_);
return v___x_678_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_write_elim(lean_object* v_motive_679_, lean_object* v_t_680_, lean_object* v_h_681_, lean_object* v_write_682_){
_start:
{
lean_object* v___x_683_; 
v___x_683_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_680_, v_write_682_);
return v___x_683_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_setD_elim___redArg(lean_object* v_t_684_, lean_object* v_setD_685_){
_start:
{
lean_object* v___x_686_; 
v___x_686_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_684_, v_setD_685_);
return v___x_686_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_setD_elim(lean_object* v_motive_687_, lean_object* v_t_688_, lean_object* v_h_689_, lean_object* v_setD_690_){
_start:
{
lean_object* v___x_691_; 
v___x_691_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_688_, v_setD_690_);
return v___x_691_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_copy_elim___redArg(lean_object* v_t_692_, lean_object* v_copy_693_){
_start:
{
lean_object* v___x_694_; 
v___x_694_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_692_, v_copy_693_);
return v___x_694_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_copy_elim(lean_object* v_motive_695_, lean_object* v_t_696_, lean_object* v_h_697_, lean_object* v_copy_698_){
_start:
{
lean_object* v___x_699_; 
v___x_699_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_696_, v_copy_698_);
return v___x_699_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpEq_elim___redArg(lean_object* v_t_700_, lean_object* v_cmpEq_701_){
_start:
{
lean_object* v___x_702_; 
v___x_702_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_700_, v_cmpEq_701_);
return v___x_702_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpEq_elim(lean_object* v_motive_703_, lean_object* v_t_704_, lean_object* v_h_705_, lean_object* v_cmpEq_706_){
_start:
{
lean_object* v___x_707_; 
v___x_707_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_704_, v_cmpEq_706_);
return v___x_707_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpNe_elim___redArg(lean_object* v_t_708_, lean_object* v_cmpNe_709_){
_start:
{
lean_object* v___x_710_; 
v___x_710_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_708_, v_cmpNe_709_);
return v___x_710_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Stmt_cmpNe_elim(lean_object* v_motive_711_, lean_object* v_t_712_, lean_object* v_h_713_, lean_object* v_cmpNe_714_){
_start:
{
lean_object* v___x_715_; 
v___x_715_ = lp_clauseflow_ClauseFlow_Stmt_ctorElim___redArg(v_t_712_, v_cmpNe_714_);
return v___x_715_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqStmt_decEq(lean_object* v_x_716_, lean_object* v_x_717_){
_start:
{
switch(lean_obj_tag(v_x_716_))
{
case 0:
{
if (lean_obj_tag(v_x_717_) == 0)
{
lean_object* v_l_718_; lean_object* v_l_719_; uint8_t v___x_720_; 
v_l_718_ = lean_ctor_get(v_x_716_, 0);
lean_inc(v_l_718_);
lean_dec_ref_known(v_x_716_, 1);
v_l_719_ = lean_ctor_get(v_x_717_, 0);
lean_inc(v_l_719_);
lean_dec_ref_known(v_x_717_, 1);
v___x_720_ = lean_nat_dec_eq(v_l_718_, v_l_719_);
lean_dec(v_l_719_);
lean_dec(v_l_718_);
return v___x_720_;
}
else
{
uint8_t v___x_721_; 
lean_dec_ref_known(v_x_716_, 1);
lean_dec_ref(v_x_717_);
v___x_721_ = 0;
return v___x_721_;
}
}
case 1:
{
if (lean_obj_tag(v_x_717_) == 1)
{
lean_object* v_same_722_; lean_object* v_same_723_; lean_object* v___x_724_; uint8_t v___x_725_; 
v_same_722_ = lean_ctor_get(v_x_716_, 0);
lean_inc(v_same_722_);
lean_dec_ref_known(v_x_716_, 1);
v_same_723_ = lean_ctor_get(v_x_717_, 0);
lean_inc(v_same_723_);
lean_dec_ref_known(v_x_717_, 1);
v___x_724_ = lean_alloc_closure((void*)(l_instDecidableEqNat___boxed), 2, 0);
v___x_725_ = l_instDecidableEqList___redArg(v___x_724_, v_same_722_, v_same_723_);
return v___x_725_;
}
else
{
uint8_t v___x_726_; 
lean_dec_ref_known(v_x_716_, 1);
lean_dec_ref(v_x_717_);
v___x_726_ = 0;
return v___x_726_;
}
}
case 2:
{
if (lean_obj_tag(v_x_717_) == 2)
{
lean_object* v_l_727_; lean_object* v_d_728_; lean_object* v_l_729_; lean_object* v_d_730_; uint8_t v___x_731_; 
v_l_727_ = lean_ctor_get(v_x_716_, 0);
lean_inc(v_l_727_);
v_d_728_ = lean_ctor_get(v_x_716_, 1);
lean_inc(v_d_728_);
lean_dec_ref_known(v_x_716_, 2);
v_l_729_ = lean_ctor_get(v_x_717_, 0);
lean_inc(v_l_729_);
v_d_730_ = lean_ctor_get(v_x_717_, 1);
lean_inc(v_d_730_);
lean_dec_ref_known(v_x_717_, 2);
v___x_731_ = lean_nat_dec_eq(v_l_727_, v_l_729_);
lean_dec(v_l_729_);
lean_dec(v_l_727_);
if (v___x_731_ == 0)
{
lean_dec(v_d_730_);
lean_dec(v_d_728_);
return v___x_731_;
}
else
{
uint8_t v___x_732_; 
v___x_732_ = lean_nat_dec_eq(v_d_728_, v_d_730_);
lean_dec(v_d_730_);
lean_dec(v_d_728_);
return v___x_732_;
}
}
else
{
uint8_t v___x_733_; 
lean_dec_ref_known(v_x_716_, 2);
lean_dec_ref(v_x_717_);
v___x_733_ = 0;
return v___x_733_;
}
}
case 3:
{
if (lean_obj_tag(v_x_717_) == 3)
{
lean_object* v_l_734_; lean_object* v_m_735_; lean_object* v_l_736_; lean_object* v_m_737_; uint8_t v___x_738_; 
v_l_734_ = lean_ctor_get(v_x_716_, 0);
lean_inc(v_l_734_);
v_m_735_ = lean_ctor_get(v_x_716_, 1);
lean_inc(v_m_735_);
lean_dec_ref_known(v_x_716_, 2);
v_l_736_ = lean_ctor_get(v_x_717_, 0);
lean_inc(v_l_736_);
v_m_737_ = lean_ctor_get(v_x_717_, 1);
lean_inc(v_m_737_);
lean_dec_ref_known(v_x_717_, 2);
v___x_738_ = lean_nat_dec_eq(v_l_734_, v_l_736_);
lean_dec(v_l_736_);
lean_dec(v_l_734_);
if (v___x_738_ == 0)
{
lean_dec(v_m_737_);
lean_dec(v_m_735_);
return v___x_738_;
}
else
{
uint8_t v___x_739_; 
v___x_739_ = lean_nat_dec_eq(v_m_735_, v_m_737_);
lean_dec(v_m_737_);
lean_dec(v_m_735_);
return v___x_739_;
}
}
else
{
uint8_t v___x_740_; 
lean_dec_ref_known(v_x_716_, 2);
lean_dec_ref(v_x_717_);
v___x_740_ = 0;
return v___x_740_;
}
}
case 4:
{
if (lean_obj_tag(v_x_717_) == 4)
{
lean_object* v_l_741_; lean_object* v_m_742_; lean_object* v_n_743_; lean_object* v_l_744_; lean_object* v_m_745_; lean_object* v_n_746_; uint8_t v___x_747_; 
v_l_741_ = lean_ctor_get(v_x_716_, 0);
lean_inc(v_l_741_);
v_m_742_ = lean_ctor_get(v_x_716_, 1);
lean_inc(v_m_742_);
v_n_743_ = lean_ctor_get(v_x_716_, 2);
lean_inc(v_n_743_);
lean_dec_ref_known(v_x_716_, 3);
v_l_744_ = lean_ctor_get(v_x_717_, 0);
lean_inc(v_l_744_);
v_m_745_ = lean_ctor_get(v_x_717_, 1);
lean_inc(v_m_745_);
v_n_746_ = lean_ctor_get(v_x_717_, 2);
lean_inc(v_n_746_);
lean_dec_ref_known(v_x_717_, 3);
v___x_747_ = lean_nat_dec_eq(v_l_741_, v_l_744_);
lean_dec(v_l_744_);
lean_dec(v_l_741_);
if (v___x_747_ == 0)
{
lean_dec(v_n_746_);
lean_dec(v_m_745_);
lean_dec(v_n_743_);
lean_dec(v_m_742_);
return v___x_747_;
}
else
{
uint8_t v___x_748_; 
v___x_748_ = lean_nat_dec_eq(v_m_742_, v_m_745_);
lean_dec(v_m_745_);
lean_dec(v_m_742_);
if (v___x_748_ == 0)
{
lean_dec(v_n_746_);
lean_dec(v_n_743_);
return v___x_748_;
}
else
{
uint8_t v___x_749_; 
v___x_749_ = lean_nat_dec_eq(v_n_743_, v_n_746_);
lean_dec(v_n_746_);
lean_dec(v_n_743_);
return v___x_749_;
}
}
}
else
{
uint8_t v___x_750_; 
lean_dec_ref_known(v_x_716_, 3);
lean_dec_ref(v_x_717_);
v___x_750_ = 0;
return v___x_750_;
}
}
default: 
{
if (lean_obj_tag(v_x_717_) == 5)
{
lean_object* v_l_751_; lean_object* v_m_752_; lean_object* v_n_753_; lean_object* v_l_754_; lean_object* v_m_755_; lean_object* v_n_756_; uint8_t v___x_757_; 
v_l_751_ = lean_ctor_get(v_x_716_, 0);
lean_inc(v_l_751_);
v_m_752_ = lean_ctor_get(v_x_716_, 1);
lean_inc(v_m_752_);
v_n_753_ = lean_ctor_get(v_x_716_, 2);
lean_inc(v_n_753_);
lean_dec_ref_known(v_x_716_, 3);
v_l_754_ = lean_ctor_get(v_x_717_, 0);
lean_inc(v_l_754_);
v_m_755_ = lean_ctor_get(v_x_717_, 1);
lean_inc(v_m_755_);
v_n_756_ = lean_ctor_get(v_x_717_, 2);
lean_inc(v_n_756_);
lean_dec_ref_known(v_x_717_, 3);
v___x_757_ = lean_nat_dec_eq(v_l_751_, v_l_754_);
lean_dec(v_l_754_);
lean_dec(v_l_751_);
if (v___x_757_ == 0)
{
lean_dec(v_n_756_);
lean_dec(v_m_755_);
lean_dec(v_n_753_);
lean_dec(v_m_752_);
return v___x_757_;
}
else
{
uint8_t v___x_758_; 
v___x_758_ = lean_nat_dec_eq(v_m_752_, v_m_755_);
lean_dec(v_m_755_);
lean_dec(v_m_752_);
if (v___x_758_ == 0)
{
lean_dec(v_n_756_);
lean_dec(v_n_753_);
return v___x_758_;
}
else
{
uint8_t v___x_759_; 
v___x_759_ = lean_nat_dec_eq(v_n_753_, v_n_756_);
lean_dec(v_n_756_);
lean_dec(v_n_753_);
return v___x_759_;
}
}
}
else
{
uint8_t v___x_760_; 
lean_dec_ref_known(v_x_716_, 3);
lean_dec_ref(v_x_717_);
v___x_760_ = 0;
return v___x_760_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqStmt_decEq___boxed(lean_object* v_x_761_, lean_object* v_x_762_){
_start:
{
uint8_t v_res_763_; lean_object* v_r_764_; 
v_res_763_ = lp_clauseflow_ClauseFlow_instDecidableEqStmt_decEq(v_x_761_, v_x_762_);
v_r_764_ = lean_box(v_res_763_);
return v_r_764_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_instDecidableEqStmt(lean_object* v_x_765_, lean_object* v_x_766_){
_start:
{
uint8_t v___x_767_; 
v___x_767_ = lp_clauseflow_ClauseFlow_instDecidableEqStmt_decEq(v_x_765_, v_x_766_);
return v___x_767_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instDecidableEqStmt___boxed(lean_object* v_x_768_, lean_object* v_x_769_){
_start:
{
uint8_t v_res_770_; lean_object* v_r_771_; 
v_res_770_ = lp_clauseflow_ClauseFlow_instDecidableEqStmt(v_x_768_, v_x_769_);
v_r_771_ = lean_box(v_res_770_);
return v_r_771_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr(lean_object* v_x_808_, lean_object* v_prec_809_){
_start:
{
switch(lean_obj_tag(v_x_808_))
{
case 0:
{
lean_object* v_l_810_; lean_object* v___x_812_; uint8_t v_isShared_813_; uint8_t v_isSharedCheck_830_; 
v_l_810_ = lean_ctor_get(v_x_808_, 0);
v_isSharedCheck_830_ = !lean_is_exclusive(v_x_808_);
if (v_isSharedCheck_830_ == 0)
{
v___x_812_ = v_x_808_;
v_isShared_813_ = v_isSharedCheck_830_;
goto v_resetjp_811_;
}
else
{
lean_inc(v_l_810_);
lean_dec(v_x_808_);
v___x_812_ = lean_box(0);
v_isShared_813_ = v_isSharedCheck_830_;
goto v_resetjp_811_;
}
v_resetjp_811_:
{
lean_object* v___y_815_; lean_object* v___x_826_; uint8_t v___x_827_; 
v___x_826_ = lean_unsigned_to_nat(1024u);
v___x_827_ = lean_nat_dec_le(v___x_826_, v_prec_809_);
if (v___x_827_ == 0)
{
lean_object* v___x_828_; 
v___x_828_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_815_ = v___x_828_;
goto v___jp_814_;
}
else
{
lean_object* v___x_829_; 
v___x_829_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_815_ = v___x_829_;
goto v___jp_814_;
}
v___jp_814_:
{
lean_object* v___x_816_; lean_object* v___x_817_; lean_object* v___x_819_; 
v___x_816_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__2));
v___x_817_ = l_Nat_reprFast(v_l_810_);
if (v_isShared_813_ == 0)
{
lean_ctor_set_tag(v___x_812_, 3);
lean_ctor_set(v___x_812_, 0, v___x_817_);
v___x_819_ = v___x_812_;
goto v_reusejp_818_;
}
else
{
lean_object* v_reuseFailAlloc_825_; 
v_reuseFailAlloc_825_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v_reuseFailAlloc_825_, 0, v___x_817_);
v___x_819_ = v_reuseFailAlloc_825_;
goto v_reusejp_818_;
}
v_reusejp_818_:
{
lean_object* v___x_820_; lean_object* v___x_821_; uint8_t v___x_822_; lean_object* v___x_823_; lean_object* v___x_824_; 
v___x_820_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_820_, 0, v___x_816_);
lean_ctor_set(v___x_820_, 1, v___x_819_);
lean_inc(v___y_815_);
v___x_821_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_821_, 0, v___y_815_);
lean_ctor_set(v___x_821_, 1, v___x_820_);
v___x_822_ = 0;
v___x_823_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_823_, 0, v___x_821_);
lean_ctor_set_uint8(v___x_823_, sizeof(void*)*1, v___x_822_);
v___x_824_ = l_Repr_addAppParen(v___x_823_, v_prec_809_);
return v___x_824_;
}
}
}
}
case 1:
{
lean_object* v_same_831_; lean_object* v___y_833_; lean_object* v___x_841_; uint8_t v___x_842_; 
v_same_831_ = lean_ctor_get(v_x_808_, 0);
lean_inc(v_same_831_);
lean_dec_ref_known(v_x_808_, 1);
v___x_841_ = lean_unsigned_to_nat(1024u);
v___x_842_ = lean_nat_dec_le(v___x_841_, v_prec_809_);
if (v___x_842_ == 0)
{
lean_object* v___x_843_; 
v___x_843_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_833_ = v___x_843_;
goto v___jp_832_;
}
else
{
lean_object* v___x_844_; 
v___x_844_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_833_ = v___x_844_;
goto v___jp_832_;
}
v___jp_832_:
{
lean_object* v___x_834_; lean_object* v___x_835_; lean_object* v___x_836_; lean_object* v___x_837_; uint8_t v___x_838_; lean_object* v___x_839_; lean_object* v___x_840_; 
v___x_834_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__5));
v___x_835_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_same_831_);
v___x_836_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_836_, 0, v___x_834_);
lean_ctor_set(v___x_836_, 1, v___x_835_);
lean_inc(v___y_833_);
v___x_837_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_837_, 0, v___y_833_);
lean_ctor_set(v___x_837_, 1, v___x_836_);
v___x_838_ = 0;
v___x_839_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_839_, 0, v___x_837_);
lean_ctor_set_uint8(v___x_839_, sizeof(void*)*1, v___x_838_);
v___x_840_ = l_Repr_addAppParen(v___x_839_, v_prec_809_);
return v___x_840_;
}
}
case 2:
{
lean_object* v_l_845_; lean_object* v_d_846_; lean_object* v___x_848_; uint8_t v_isShared_849_; uint8_t v_isSharedCheck_871_; 
v_l_845_ = lean_ctor_get(v_x_808_, 0);
v_d_846_ = lean_ctor_get(v_x_808_, 1);
v_isSharedCheck_871_ = !lean_is_exclusive(v_x_808_);
if (v_isSharedCheck_871_ == 0)
{
v___x_848_ = v_x_808_;
v_isShared_849_ = v_isSharedCheck_871_;
goto v_resetjp_847_;
}
else
{
lean_inc(v_d_846_);
lean_inc(v_l_845_);
lean_dec(v_x_808_);
v___x_848_ = lean_box(0);
v_isShared_849_ = v_isSharedCheck_871_;
goto v_resetjp_847_;
}
v_resetjp_847_:
{
lean_object* v___y_851_; lean_object* v___x_867_; uint8_t v___x_868_; 
v___x_867_ = lean_unsigned_to_nat(1024u);
v___x_868_ = lean_nat_dec_le(v___x_867_, v_prec_809_);
if (v___x_868_ == 0)
{
lean_object* v___x_869_; 
v___x_869_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_851_ = v___x_869_;
goto v___jp_850_;
}
else
{
lean_object* v___x_870_; 
v___x_870_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_851_ = v___x_870_;
goto v___jp_850_;
}
v___jp_850_:
{
lean_object* v___x_852_; lean_object* v___x_853_; lean_object* v___x_854_; lean_object* v___x_855_; lean_object* v___x_857_; 
v___x_852_ = lean_box(1);
v___x_853_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__8));
v___x_854_ = l_Nat_reprFast(v_l_845_);
v___x_855_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_855_, 0, v___x_854_);
if (v_isShared_849_ == 0)
{
lean_ctor_set_tag(v___x_848_, 5);
lean_ctor_set(v___x_848_, 1, v___x_855_);
lean_ctor_set(v___x_848_, 0, v___x_853_);
v___x_857_ = v___x_848_;
goto v_reusejp_856_;
}
else
{
lean_object* v_reuseFailAlloc_866_; 
v_reuseFailAlloc_866_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_866_, 0, v___x_853_);
lean_ctor_set(v_reuseFailAlloc_866_, 1, v___x_855_);
v___x_857_ = v_reuseFailAlloc_866_;
goto v_reusejp_856_;
}
v_reusejp_856_:
{
lean_object* v___x_858_; lean_object* v___x_859_; lean_object* v___x_860_; lean_object* v___x_861_; lean_object* v___x_862_; uint8_t v___x_863_; lean_object* v___x_864_; lean_object* v___x_865_; 
v___x_858_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_858_, 0, v___x_857_);
lean_ctor_set(v___x_858_, 1, v___x_852_);
v___x_859_ = l_Nat_reprFast(v_d_846_);
v___x_860_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_860_, 0, v___x_859_);
v___x_861_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_861_, 0, v___x_858_);
lean_ctor_set(v___x_861_, 1, v___x_860_);
lean_inc(v___y_851_);
v___x_862_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_862_, 0, v___y_851_);
lean_ctor_set(v___x_862_, 1, v___x_861_);
v___x_863_ = 0;
v___x_864_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_864_, 0, v___x_862_);
lean_ctor_set_uint8(v___x_864_, sizeof(void*)*1, v___x_863_);
v___x_865_ = l_Repr_addAppParen(v___x_864_, v_prec_809_);
return v___x_865_;
}
}
}
}
case 3:
{
lean_object* v_l_872_; lean_object* v_m_873_; lean_object* v___x_875_; uint8_t v_isShared_876_; uint8_t v_isSharedCheck_898_; 
v_l_872_ = lean_ctor_get(v_x_808_, 0);
v_m_873_ = lean_ctor_get(v_x_808_, 1);
v_isSharedCheck_898_ = !lean_is_exclusive(v_x_808_);
if (v_isSharedCheck_898_ == 0)
{
v___x_875_ = v_x_808_;
v_isShared_876_ = v_isSharedCheck_898_;
goto v_resetjp_874_;
}
else
{
lean_inc(v_m_873_);
lean_inc(v_l_872_);
lean_dec(v_x_808_);
v___x_875_ = lean_box(0);
v_isShared_876_ = v_isSharedCheck_898_;
goto v_resetjp_874_;
}
v_resetjp_874_:
{
lean_object* v___y_878_; lean_object* v___x_894_; uint8_t v___x_895_; 
v___x_894_ = lean_unsigned_to_nat(1024u);
v___x_895_ = lean_nat_dec_le(v___x_894_, v_prec_809_);
if (v___x_895_ == 0)
{
lean_object* v___x_896_; 
v___x_896_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_878_ = v___x_896_;
goto v___jp_877_;
}
else
{
lean_object* v___x_897_; 
v___x_897_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_878_ = v___x_897_;
goto v___jp_877_;
}
v___jp_877_:
{
lean_object* v___x_879_; lean_object* v___x_880_; lean_object* v___x_881_; lean_object* v___x_882_; lean_object* v___x_884_; 
v___x_879_ = lean_box(1);
v___x_880_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__11));
v___x_881_ = l_Nat_reprFast(v_l_872_);
v___x_882_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_882_, 0, v___x_881_);
if (v_isShared_876_ == 0)
{
lean_ctor_set_tag(v___x_875_, 5);
lean_ctor_set(v___x_875_, 1, v___x_882_);
lean_ctor_set(v___x_875_, 0, v___x_880_);
v___x_884_ = v___x_875_;
goto v_reusejp_883_;
}
else
{
lean_object* v_reuseFailAlloc_893_; 
v_reuseFailAlloc_893_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_893_, 0, v___x_880_);
lean_ctor_set(v_reuseFailAlloc_893_, 1, v___x_882_);
v___x_884_ = v_reuseFailAlloc_893_;
goto v_reusejp_883_;
}
v_reusejp_883_:
{
lean_object* v___x_885_; lean_object* v___x_886_; lean_object* v___x_887_; lean_object* v___x_888_; lean_object* v___x_889_; uint8_t v___x_890_; lean_object* v___x_891_; lean_object* v___x_892_; 
v___x_885_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_885_, 0, v___x_884_);
lean_ctor_set(v___x_885_, 1, v___x_879_);
v___x_886_ = l_Nat_reprFast(v_m_873_);
v___x_887_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_887_, 0, v___x_886_);
v___x_888_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_888_, 0, v___x_885_);
lean_ctor_set(v___x_888_, 1, v___x_887_);
lean_inc(v___y_878_);
v___x_889_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_889_, 0, v___y_878_);
lean_ctor_set(v___x_889_, 1, v___x_888_);
v___x_890_ = 0;
v___x_891_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_891_, 0, v___x_889_);
lean_ctor_set_uint8(v___x_891_, sizeof(void*)*1, v___x_890_);
v___x_892_ = l_Repr_addAppParen(v___x_891_, v_prec_809_);
return v___x_892_;
}
}
}
}
case 4:
{
lean_object* v_l_899_; lean_object* v_m_900_; lean_object* v_n_901_; lean_object* v___y_903_; lean_object* v___x_921_; uint8_t v___x_922_; 
v_l_899_ = lean_ctor_get(v_x_808_, 0);
lean_inc(v_l_899_);
v_m_900_ = lean_ctor_get(v_x_808_, 1);
lean_inc(v_m_900_);
v_n_901_ = lean_ctor_get(v_x_808_, 2);
lean_inc(v_n_901_);
lean_dec_ref_known(v_x_808_, 3);
v___x_921_ = lean_unsigned_to_nat(1024u);
v___x_922_ = lean_nat_dec_le(v___x_921_, v_prec_809_);
if (v___x_922_ == 0)
{
lean_object* v___x_923_; 
v___x_923_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_903_ = v___x_923_;
goto v___jp_902_;
}
else
{
lean_object* v___x_924_; 
v___x_924_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_903_ = v___x_924_;
goto v___jp_902_;
}
v___jp_902_:
{
lean_object* v___x_904_; lean_object* v___x_905_; lean_object* v___x_906_; lean_object* v___x_907_; lean_object* v___x_908_; lean_object* v___x_909_; lean_object* v___x_910_; lean_object* v___x_911_; lean_object* v___x_912_; lean_object* v___x_913_; lean_object* v___x_914_; lean_object* v___x_915_; lean_object* v___x_916_; lean_object* v___x_917_; uint8_t v___x_918_; lean_object* v___x_919_; lean_object* v___x_920_; 
v___x_904_ = lean_box(1);
v___x_905_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__14));
v___x_906_ = l_Nat_reprFast(v_l_899_);
v___x_907_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_907_, 0, v___x_906_);
v___x_908_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_908_, 0, v___x_905_);
lean_ctor_set(v___x_908_, 1, v___x_907_);
v___x_909_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_909_, 0, v___x_908_);
lean_ctor_set(v___x_909_, 1, v___x_904_);
v___x_910_ = l_Nat_reprFast(v_m_900_);
v___x_911_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_911_, 0, v___x_910_);
v___x_912_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_912_, 0, v___x_909_);
lean_ctor_set(v___x_912_, 1, v___x_911_);
v___x_913_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_913_, 0, v___x_912_);
lean_ctor_set(v___x_913_, 1, v___x_904_);
v___x_914_ = l_Nat_reprFast(v_n_901_);
v___x_915_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_915_, 0, v___x_914_);
v___x_916_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_916_, 0, v___x_913_);
lean_ctor_set(v___x_916_, 1, v___x_915_);
lean_inc(v___y_903_);
v___x_917_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_917_, 0, v___y_903_);
lean_ctor_set(v___x_917_, 1, v___x_916_);
v___x_918_ = 0;
v___x_919_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_919_, 0, v___x_917_);
lean_ctor_set_uint8(v___x_919_, sizeof(void*)*1, v___x_918_);
v___x_920_ = l_Repr_addAppParen(v___x_919_, v_prec_809_);
return v___x_920_;
}
}
default: 
{
lean_object* v_l_925_; lean_object* v_m_926_; lean_object* v_n_927_; lean_object* v___y_929_; lean_object* v___x_947_; uint8_t v___x_948_; 
v_l_925_ = lean_ctor_get(v_x_808_, 0);
lean_inc(v_l_925_);
v_m_926_ = lean_ctor_get(v_x_808_, 1);
lean_inc(v_m_926_);
v_n_927_ = lean_ctor_get(v_x_808_, 2);
lean_inc(v_n_927_);
lean_dec_ref_known(v_x_808_, 3);
v___x_947_ = lean_unsigned_to_nat(1024u);
v___x_948_ = lean_nat_dec_le(v___x_947_, v_prec_809_);
if (v___x_948_ == 0)
{
lean_object* v___x_949_; 
v___x_949_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_929_ = v___x_949_;
goto v___jp_928_;
}
else
{
lean_object* v___x_950_; 
v___x_950_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_929_ = v___x_950_;
goto v___jp_928_;
}
v___jp_928_:
{
lean_object* v___x_930_; lean_object* v___x_931_; lean_object* v___x_932_; lean_object* v___x_933_; lean_object* v___x_934_; lean_object* v___x_935_; lean_object* v___x_936_; lean_object* v___x_937_; lean_object* v___x_938_; lean_object* v___x_939_; lean_object* v___x_940_; lean_object* v___x_941_; lean_object* v___x_942_; lean_object* v___x_943_; uint8_t v___x_944_; lean_object* v___x_945_; lean_object* v___x_946_; 
v___x_930_ = lean_box(1);
v___x_931_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprStmt_repr___closed__17));
v___x_932_ = l_Nat_reprFast(v_l_925_);
v___x_933_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_933_, 0, v___x_932_);
v___x_934_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_934_, 0, v___x_931_);
lean_ctor_set(v___x_934_, 1, v___x_933_);
v___x_935_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_935_, 0, v___x_934_);
lean_ctor_set(v___x_935_, 1, v___x_930_);
v___x_936_ = l_Nat_reprFast(v_m_926_);
v___x_937_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_937_, 0, v___x_936_);
v___x_938_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_938_, 0, v___x_935_);
lean_ctor_set(v___x_938_, 1, v___x_937_);
v___x_939_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_939_, 0, v___x_938_);
lean_ctor_set(v___x_939_, 1, v___x_930_);
v___x_940_ = l_Nat_reprFast(v_n_927_);
v___x_941_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_941_, 0, v___x_940_);
v___x_942_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_942_, 0, v___x_939_);
lean_ctor_set(v___x_942_, 1, v___x_941_);
lean_inc(v___y_929_);
v___x_943_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_943_, 0, v___y_929_);
lean_ctor_set(v___x_943_, 1, v___x_942_);
v___x_944_ = 0;
v___x_945_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_945_, 0, v___x_943_);
lean_ctor_set_uint8(v___x_945_, sizeof(void*)*1, v___x_944_);
v___x_946_ = l_Repr_addAppParen(v___x_945_, v_prec_809_);
return v___x_946_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprStmt_repr___boxed(lean_object* v_x_951_, lean_object* v_prec_952_){
_start:
{
lean_object* v_res_953_; 
v_res_953_ = lp_clauseflow_ClauseFlow_instReprStmt_repr(v_x_951_, v_prec_952_);
lean_dec(v_prec_952_);
return v_res_953_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorIdx(lean_object* v_x_956_){
_start:
{
switch(lean_obj_tag(v_x_956_))
{
case 0:
{
lean_object* v___x_957_; 
v___x_957_ = lean_unsigned_to_nat(0u);
return v___x_957_;
}
case 1:
{
lean_object* v___x_958_; 
v___x_958_ = lean_unsigned_to_nat(1u);
return v___x_958_;
}
case 2:
{
lean_object* v___x_959_; 
v___x_959_ = lean_unsigned_to_nat(2u);
return v___x_959_;
}
case 3:
{
lean_object* v___x_960_; 
v___x_960_ = lean_unsigned_to_nat(3u);
return v___x_960_;
}
case 4:
{
lean_object* v___x_961_; 
v___x_961_ = lean_unsigned_to_nat(4u);
return v___x_961_;
}
default: 
{
lean_object* v___x_962_; 
v___x_962_ = lean_unsigned_to_nat(5u);
return v___x_962_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorIdx___boxed(lean_object* v_x_963_){
_start:
{
lean_object* v_res_964_; 
v_res_964_ = lp_clauseflow_ClauseFlow_Term_ctorIdx(v_x_963_);
lean_dec(v_x_963_);
return v_res_964_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(lean_object* v_t_965_, lean_object* v_k_966_){
_start:
{
switch(lean_obj_tag(v_t_965_))
{
case 0:
{
lean_object* v_b_967_; lean_object* v___x_968_; 
v_b_967_ = lean_ctor_get(v_t_965_, 0);
lean_inc(v_b_967_);
lean_dec_ref_known(v_t_965_, 1);
v___x_968_ = lean_apply_1(v_k_966_, v_b_967_);
return v___x_968_;
}
case 1:
{
lean_object* v_l_969_; lean_object* v_cases_970_; lean_object* v_other_971_; lean_object* v___x_972_; 
v_l_969_ = lean_ctor_get(v_t_965_, 0);
lean_inc(v_l_969_);
v_cases_970_ = lean_ctor_get(v_t_965_, 1);
lean_inc(v_cases_970_);
v_other_971_ = lean_ctor_get(v_t_965_, 2);
lean_inc(v_other_971_);
lean_dec_ref_known(v_t_965_, 3);
v___x_972_ = lean_apply_3(v_k_966_, v_l_969_, v_cases_970_, v_other_971_);
return v___x_972_;
}
case 2:
{
lean_object* v_dsts_973_; lean_object* v_b_974_; lean_object* v_same_975_; lean_object* v_post_976_; lean_object* v___x_977_; 
v_dsts_973_ = lean_ctor_get(v_t_965_, 0);
lean_inc(v_dsts_973_);
v_b_974_ = lean_ctor_get(v_t_965_, 1);
lean_inc(v_b_974_);
v_same_975_ = lean_ctor_get(v_t_965_, 2);
lean_inc(v_same_975_);
v_post_976_ = lean_ctor_get(v_t_965_, 3);
lean_inc(v_post_976_);
lean_dec_ref_known(v_t_965_, 4);
v___x_977_ = lean_apply_4(v_k_966_, v_dsts_973_, v_b_974_, v_same_975_, v_post_976_);
return v___x_977_;
}
case 3:
{
lean_object* v_dst_978_; lean_object* v_src_979_; lean_object* v_pd_980_; lean_object* v_ps_981_; lean_object* v_b_982_; lean_object* v___x_983_; 
v_dst_978_ = lean_ctor_get(v_t_965_, 0);
lean_inc(v_dst_978_);
v_src_979_ = lean_ctor_get(v_t_965_, 1);
lean_inc(v_src_979_);
v_pd_980_ = lean_ctor_get(v_t_965_, 2);
lean_inc(v_pd_980_);
v_ps_981_ = lean_ctor_get(v_t_965_, 3);
lean_inc(v_ps_981_);
v_b_982_ = lean_ctor_get(v_t_965_, 4);
lean_inc(v_b_982_);
lean_dec_ref_known(v_t_965_, 5);
v___x_983_ = lean_apply_5(v_k_966_, v_dst_978_, v_src_979_, v_pd_980_, v_ps_981_, v_b_982_);
return v___x_983_;
}
default: 
{
lean_dec(v_t_965_);
return v_k_966_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorElim(lean_object* v_motive_984_, lean_object* v_ctorIdx_985_, lean_object* v_t_986_, lean_object* v_h_987_, lean_object* v_k_988_){
_start:
{
lean_object* v___x_989_; 
v___x_989_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_986_, v_k_988_);
return v___x_989_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ctorElim___boxed(lean_object* v_motive_990_, lean_object* v_ctorIdx_991_, lean_object* v_t_992_, lean_object* v_h_993_, lean_object* v_k_994_){
_start:
{
lean_object* v_res_995_; 
v_res_995_ = lp_clauseflow_ClauseFlow_Term_ctorElim(v_motive_990_, v_ctorIdx_991_, v_t_992_, v_h_993_, v_k_994_);
lean_dec(v_ctorIdx_991_);
return v_res_995_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_goto_elim___redArg(lean_object* v_t_996_, lean_object* v_goto_997_){
_start:
{
lean_object* v___x_998_; 
v___x_998_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_996_, v_goto_997_);
return v___x_998_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_goto_elim(lean_object* v_motive_999_, lean_object* v_t_1000_, lean_object* v_h_1001_, lean_object* v_goto_1002_){
_start:
{
lean_object* v___x_1003_; 
v___x_1003_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1000_, v_goto_1002_);
return v___x_1003_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_switch_elim___redArg(lean_object* v_t_1004_, lean_object* v_switch_1005_){
_start:
{
lean_object* v___x_1006_; 
v___x_1006_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1004_, v_switch_1005_);
return v___x_1006_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_switch_elim(lean_object* v_motive_1007_, lean_object* v_t_1008_, lean_object* v_h_1009_, lean_object* v_switch_1010_){
_start:
{
lean_object* v___x_1011_; 
v___x_1011_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1008_, v_switch_1010_);
return v___x_1011_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_call_elim___redArg(lean_object* v_t_1012_, lean_object* v_call_1013_){
_start:
{
lean_object* v___x_1014_; 
v___x_1014_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1012_, v_call_1013_);
return v___x_1014_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_call_elim(lean_object* v_motive_1015_, lean_object* v_t_1016_, lean_object* v_h_1017_, lean_object* v_call_1018_){
_start:
{
lean_object* v___x_1019_; 
v___x_1019_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1016_, v_call_1018_);
return v___x_1019_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_branch_elim___redArg(lean_object* v_t_1020_, lean_object* v_branch_1021_){
_start:
{
lean_object* v___x_1022_; 
v___x_1022_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1020_, v_branch_1021_);
return v___x_1022_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_branch_elim(lean_object* v_motive_1023_, lean_object* v_t_1024_, lean_object* v_h_1025_, lean_object* v_branch_1026_){
_start:
{
lean_object* v___x_1027_; 
v___x_1027_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1024_, v_branch_1026_);
return v___x_1027_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ret_elim___redArg(lean_object* v_t_1028_, lean_object* v_ret_1029_){
_start:
{
lean_object* v___x_1030_; 
v___x_1030_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1028_, v_ret_1029_);
return v___x_1030_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_ret_elim(lean_object* v_motive_1031_, lean_object* v_t_1032_, lean_object* v_h_1033_, lean_object* v_ret_1034_){
_start:
{
lean_object* v___x_1035_; 
v___x_1035_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1032_, v_ret_1034_);
return v___x_1035_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_stop_elim___redArg(lean_object* v_t_1036_, lean_object* v_stop_1037_){
_start:
{
lean_object* v___x_1038_; 
v___x_1038_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1036_, v_stop_1037_);
return v___x_1038_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Term_stop_elim(lean_object* v_motive_1039_, lean_object* v_t_1040_, lean_object* v_h_1041_, lean_object* v_stop_1042_){
_start:
{
lean_object* v___x_1043_; 
v___x_1043_ = lp_clauseflow_ClauseFlow_Term_ctorElim___redArg(v_t_1040_, v_stop_1042_);
return v___x_1043_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3_spec__5_spec__7(lean_object* v_x_1044_, lean_object* v_x_1045_, lean_object* v_x_1046_){
_start:
{
if (lean_obj_tag(v_x_1046_) == 0)
{
lean_dec(v_x_1044_);
return v_x_1045_;
}
else
{
lean_object* v_head_1047_; lean_object* v_tail_1048_; lean_object* v___x_1050_; uint8_t v_isShared_1051_; uint8_t v_isSharedCheck_1058_; 
v_head_1047_ = lean_ctor_get(v_x_1046_, 0);
v_tail_1048_ = lean_ctor_get(v_x_1046_, 1);
v_isSharedCheck_1058_ = !lean_is_exclusive(v_x_1046_);
if (v_isSharedCheck_1058_ == 0)
{
v___x_1050_ = v_x_1046_;
v_isShared_1051_ = v_isSharedCheck_1058_;
goto v_resetjp_1049_;
}
else
{
lean_inc(v_tail_1048_);
lean_inc(v_head_1047_);
lean_dec(v_x_1046_);
v___x_1050_ = lean_box(0);
v_isShared_1051_ = v_isSharedCheck_1058_;
goto v_resetjp_1049_;
}
v_resetjp_1049_:
{
lean_object* v___x_1053_; 
lean_inc(v_x_1044_);
if (v_isShared_1051_ == 0)
{
lean_ctor_set_tag(v___x_1050_, 5);
lean_ctor_set(v___x_1050_, 1, v_x_1044_);
lean_ctor_set(v___x_1050_, 0, v_x_1045_);
v___x_1053_ = v___x_1050_;
goto v_reusejp_1052_;
}
else
{
lean_object* v_reuseFailAlloc_1057_; 
v_reuseFailAlloc_1057_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1057_, 0, v_x_1045_);
lean_ctor_set(v_reuseFailAlloc_1057_, 1, v_x_1044_);
v___x_1053_ = v_reuseFailAlloc_1057_;
goto v_reusejp_1052_;
}
v_reusejp_1052_:
{
lean_object* v___x_1054_; lean_object* v___x_1055_; 
v___x_1054_ = lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(v_head_1047_);
v___x_1055_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1055_, 0, v___x_1053_);
lean_ctor_set(v___x_1055_, 1, v___x_1054_);
v_x_1045_ = v___x_1055_;
v_x_1046_ = v_tail_1048_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3_spec__5(lean_object* v_x_1059_, lean_object* v_x_1060_, lean_object* v_x_1061_){
_start:
{
if (lean_obj_tag(v_x_1061_) == 0)
{
lean_dec(v_x_1059_);
return v_x_1060_;
}
else
{
lean_object* v_head_1062_; lean_object* v_tail_1063_; lean_object* v___x_1065_; uint8_t v_isShared_1066_; uint8_t v_isSharedCheck_1073_; 
v_head_1062_ = lean_ctor_get(v_x_1061_, 0);
v_tail_1063_ = lean_ctor_get(v_x_1061_, 1);
v_isSharedCheck_1073_ = !lean_is_exclusive(v_x_1061_);
if (v_isSharedCheck_1073_ == 0)
{
v___x_1065_ = v_x_1061_;
v_isShared_1066_ = v_isSharedCheck_1073_;
goto v_resetjp_1064_;
}
else
{
lean_inc(v_tail_1063_);
lean_inc(v_head_1062_);
lean_dec(v_x_1061_);
v___x_1065_ = lean_box(0);
v_isShared_1066_ = v_isSharedCheck_1073_;
goto v_resetjp_1064_;
}
v_resetjp_1064_:
{
lean_object* v___x_1068_; 
lean_inc(v_x_1059_);
if (v_isShared_1066_ == 0)
{
lean_ctor_set_tag(v___x_1065_, 5);
lean_ctor_set(v___x_1065_, 1, v_x_1059_);
lean_ctor_set(v___x_1065_, 0, v_x_1060_);
v___x_1068_ = v___x_1065_;
goto v_reusejp_1067_;
}
else
{
lean_object* v_reuseFailAlloc_1072_; 
v_reuseFailAlloc_1072_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1072_, 0, v_x_1060_);
lean_ctor_set(v_reuseFailAlloc_1072_, 1, v_x_1059_);
v___x_1068_ = v_reuseFailAlloc_1072_;
goto v_reusejp_1067_;
}
v_reusejp_1067_:
{
lean_object* v___x_1069_; lean_object* v___x_1070_; lean_object* v___x_1071_; 
v___x_1069_ = lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(v_head_1062_);
v___x_1070_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1070_, 0, v___x_1068_);
lean_ctor_set(v___x_1070_, 1, v___x_1069_);
v___x_1071_ = lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3_spec__5_spec__7(v_x_1059_, v___x_1070_, v_tail_1063_);
return v___x_1071_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3(lean_object* v_x_1074_, lean_object* v_x_1075_){
_start:
{
if (lean_obj_tag(v_x_1074_) == 0)
{
lean_object* v___x_1076_; 
lean_dec(v_x_1075_);
v___x_1076_ = lean_box(0);
return v___x_1076_;
}
else
{
lean_object* v_tail_1077_; 
v_tail_1077_ = lean_ctor_get(v_x_1074_, 1);
if (lean_obj_tag(v_tail_1077_) == 0)
{
lean_object* v_head_1078_; lean_object* v___x_1079_; 
lean_dec(v_x_1075_);
v_head_1078_ = lean_ctor_get(v_x_1074_, 0);
lean_inc(v_head_1078_);
lean_dec_ref_known(v_x_1074_, 2);
v___x_1079_ = lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(v_head_1078_);
return v___x_1079_;
}
else
{
lean_object* v_head_1080_; lean_object* v___x_1081_; lean_object* v___x_1082_; 
lean_inc(v_tail_1077_);
v_head_1080_ = lean_ctor_get(v_x_1074_, 0);
lean_inc(v_head_1080_);
lean_dec_ref_known(v_x_1074_, 2);
v___x_1081_ = lp_clauseflow_ClauseFlow_instReprClause_repr___redArg(v_head_1080_);
v___x_1082_ = lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3_spec__5(v_x_1075_, v___x_1081_, v_tail_1077_);
return v___x_1082_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___redArg(lean_object* v_a_1083_){
_start:
{
if (lean_obj_tag(v_a_1083_) == 0)
{
lean_object* v___x_1084_; 
v___x_1084_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1));
return v___x_1084_;
}
else
{
lean_object* v___x_1085_; lean_object* v___x_1086_; lean_object* v___x_1087_; lean_object* v___x_1088_; lean_object* v___x_1089_; lean_object* v___x_1090_; lean_object* v___x_1091_; lean_object* v___x_1092_; uint8_t v___x_1093_; lean_object* v___x_1094_; 
v___x_1085_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_1086_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1_spec__3(v_a_1083_, v___x_1085_);
v___x_1087_ = lean_obj_once(&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8, &lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8_once, _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8);
v___x_1088_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9));
v___x_1089_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1089_, 0, v___x_1088_);
lean_ctor_set(v___x_1089_, 1, v___x_1086_);
v___x_1090_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10));
v___x_1091_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1091_, 0, v___x_1089_);
lean_ctor_set(v___x_1091_, 1, v___x_1090_);
v___x_1092_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1092_, 0, v___x_1087_);
lean_ctor_set(v___x_1092_, 1, v___x_1091_);
v___x_1093_ = 0;
v___x_1094_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1094_, 0, v___x_1092_);
lean_ctor_set_uint8(v___x_1094_, sizeof(void*)*1, v___x_1093_);
return v___x_1094_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(lean_object* v_x_1095_){
_start:
{
lean_object* v_fst_1096_; lean_object* v_snd_1097_; lean_object* v___x_1099_; uint8_t v_isShared_1100_; uint8_t v_isSharedCheck_1121_; 
v_fst_1096_ = lean_ctor_get(v_x_1095_, 0);
v_snd_1097_ = lean_ctor_get(v_x_1095_, 1);
v_isSharedCheck_1121_ = !lean_is_exclusive(v_x_1095_);
if (v_isSharedCheck_1121_ == 0)
{
v___x_1099_ = v_x_1095_;
v_isShared_1100_ = v_isSharedCheck_1121_;
goto v_resetjp_1098_;
}
else
{
lean_inc(v_snd_1097_);
lean_inc(v_fst_1096_);
lean_dec(v_x_1095_);
v___x_1099_ = lean_box(0);
v_isShared_1100_ = v_isSharedCheck_1121_;
goto v_resetjp_1098_;
}
v_resetjp_1098_:
{
lean_object* v___x_1101_; lean_object* v___x_1102_; lean_object* v___x_1103_; lean_object* v___x_1105_; 
v___x_1101_ = l_Nat_reprFast(v_fst_1096_);
v___x_1102_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1102_, 0, v___x_1101_);
v___x_1103_ = lean_box(0);
if (v_isShared_1100_ == 0)
{
lean_ctor_set_tag(v___x_1099_, 1);
lean_ctor_set(v___x_1099_, 1, v___x_1103_);
lean_ctor_set(v___x_1099_, 0, v___x_1102_);
v___x_1105_ = v___x_1099_;
goto v_reusejp_1104_;
}
else
{
lean_object* v_reuseFailAlloc_1120_; 
v_reuseFailAlloc_1120_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1120_, 0, v___x_1102_);
lean_ctor_set(v_reuseFailAlloc_1120_, 1, v___x_1103_);
v___x_1105_ = v_reuseFailAlloc_1120_;
goto v_reusejp_1104_;
}
v_reusejp_1104_:
{
lean_object* v___x_1106_; lean_object* v___x_1107_; lean_object* v___x_1108_; lean_object* v___x_1109_; lean_object* v___x_1110_; lean_object* v___x_1111_; lean_object* v___x_1112_; lean_object* v___x_1113_; lean_object* v___x_1114_; lean_object* v___x_1115_; lean_object* v___x_1116_; lean_object* v___x_1117_; uint8_t v___x_1118_; lean_object* v___x_1119_; 
v___x_1106_ = l_Nat_reprFast(v_snd_1097_);
v___x_1107_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1107_, 0, v___x_1106_);
v___x_1108_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_1108_, 0, v___x_1107_);
lean_ctor_set(v___x_1108_, 1, v___x_1105_);
v___x_1109_ = l_List_reverse___redArg(v___x_1108_);
v___x_1110_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_1111_ = l_Std_Format_joinSep___at___00Lean_Syntax_formatStxAux_spec__2(v___x_1109_, v___x_1110_);
v___x_1112_ = lean_obj_once(&lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3, &lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3_once, _init_lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__3);
v___x_1113_ = ((lean_object*)(lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__4));
v___x_1114_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1114_, 0, v___x_1113_);
lean_ctor_set(v___x_1114_, 1, v___x_1111_);
v___x_1115_ = ((lean_object*)(lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprClause_repr_spec__0_spec__0___redArg___closed__5));
v___x_1116_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1116_, 0, v___x_1114_);
lean_ctor_set(v___x_1116_, 1, v___x_1115_);
v___x_1117_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1117_, 0, v___x_1112_);
lean_ctor_set(v___x_1117_, 1, v___x_1116_);
v___x_1118_ = 0;
v___x_1119_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1119_, 0, v___x_1117_);
lean_ctor_set_uint8(v___x_1119_, sizeof(void*)*1, v___x_1118_);
return v___x_1119_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1_spec__2_spec__4(lean_object* v_x_1122_, lean_object* v_x_1123_, lean_object* v_x_1124_){
_start:
{
if (lean_obj_tag(v_x_1124_) == 0)
{
lean_dec(v_x_1122_);
return v_x_1123_;
}
else
{
lean_object* v_head_1125_; lean_object* v_tail_1126_; lean_object* v___x_1128_; uint8_t v_isShared_1129_; uint8_t v_isSharedCheck_1136_; 
v_head_1125_ = lean_ctor_get(v_x_1124_, 0);
v_tail_1126_ = lean_ctor_get(v_x_1124_, 1);
v_isSharedCheck_1136_ = !lean_is_exclusive(v_x_1124_);
if (v_isSharedCheck_1136_ == 0)
{
v___x_1128_ = v_x_1124_;
v_isShared_1129_ = v_isSharedCheck_1136_;
goto v_resetjp_1127_;
}
else
{
lean_inc(v_tail_1126_);
lean_inc(v_head_1125_);
lean_dec(v_x_1124_);
v___x_1128_ = lean_box(0);
v_isShared_1129_ = v_isSharedCheck_1136_;
goto v_resetjp_1127_;
}
v_resetjp_1127_:
{
lean_object* v___x_1131_; 
lean_inc(v_x_1122_);
if (v_isShared_1129_ == 0)
{
lean_ctor_set_tag(v___x_1128_, 5);
lean_ctor_set(v___x_1128_, 1, v_x_1122_);
lean_ctor_set(v___x_1128_, 0, v_x_1123_);
v___x_1131_ = v___x_1128_;
goto v_reusejp_1130_;
}
else
{
lean_object* v_reuseFailAlloc_1135_; 
v_reuseFailAlloc_1135_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1135_, 0, v_x_1123_);
lean_ctor_set(v_reuseFailAlloc_1135_, 1, v_x_1122_);
v___x_1131_ = v_reuseFailAlloc_1135_;
goto v_reusejp_1130_;
}
v_reusejp_1130_:
{
lean_object* v___x_1132_; lean_object* v___x_1133_; 
v___x_1132_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(v_head_1125_);
v___x_1133_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1133_, 0, v___x_1131_);
lean_ctor_set(v___x_1133_, 1, v___x_1132_);
v_x_1123_ = v___x_1133_;
v_x_1124_ = v_tail_1126_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1_spec__2(lean_object* v_x_1137_, lean_object* v_x_1138_, lean_object* v_x_1139_){
_start:
{
if (lean_obj_tag(v_x_1139_) == 0)
{
lean_dec(v_x_1137_);
return v_x_1138_;
}
else
{
lean_object* v_head_1140_; lean_object* v_tail_1141_; lean_object* v___x_1143_; uint8_t v_isShared_1144_; uint8_t v_isSharedCheck_1151_; 
v_head_1140_ = lean_ctor_get(v_x_1139_, 0);
v_tail_1141_ = lean_ctor_get(v_x_1139_, 1);
v_isSharedCheck_1151_ = !lean_is_exclusive(v_x_1139_);
if (v_isSharedCheck_1151_ == 0)
{
v___x_1143_ = v_x_1139_;
v_isShared_1144_ = v_isSharedCheck_1151_;
goto v_resetjp_1142_;
}
else
{
lean_inc(v_tail_1141_);
lean_inc(v_head_1140_);
lean_dec(v_x_1139_);
v___x_1143_ = lean_box(0);
v_isShared_1144_ = v_isSharedCheck_1151_;
goto v_resetjp_1142_;
}
v_resetjp_1142_:
{
lean_object* v___x_1146_; 
lean_inc(v_x_1137_);
if (v_isShared_1144_ == 0)
{
lean_ctor_set_tag(v___x_1143_, 5);
lean_ctor_set(v___x_1143_, 1, v_x_1137_);
lean_ctor_set(v___x_1143_, 0, v_x_1138_);
v___x_1146_ = v___x_1143_;
goto v_reusejp_1145_;
}
else
{
lean_object* v_reuseFailAlloc_1150_; 
v_reuseFailAlloc_1150_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1150_, 0, v_x_1138_);
lean_ctor_set(v_reuseFailAlloc_1150_, 1, v_x_1137_);
v___x_1146_ = v_reuseFailAlloc_1150_;
goto v_reusejp_1145_;
}
v_reusejp_1145_:
{
lean_object* v___x_1147_; lean_object* v___x_1148_; lean_object* v___x_1149_; 
v___x_1147_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(v_head_1140_);
v___x_1148_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1148_, 0, v___x_1146_);
lean_ctor_set(v___x_1148_, 1, v___x_1147_);
v___x_1149_ = lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1_spec__2_spec__4(v_x_1137_, v___x_1148_, v_tail_1141_);
return v___x_1149_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1(lean_object* v_x_1152_, lean_object* v_x_1153_){
_start:
{
if (lean_obj_tag(v_x_1152_) == 0)
{
lean_object* v___x_1154_; 
lean_dec(v_x_1153_);
v___x_1154_ = lean_box(0);
return v___x_1154_;
}
else
{
lean_object* v_tail_1155_; 
v_tail_1155_ = lean_ctor_get(v_x_1152_, 1);
if (lean_obj_tag(v_tail_1155_) == 0)
{
lean_object* v_head_1156_; lean_object* v___x_1157_; 
lean_dec(v_x_1153_);
v_head_1156_ = lean_ctor_get(v_x_1152_, 0);
lean_inc(v_head_1156_);
lean_dec_ref_known(v_x_1152_, 2);
v___x_1157_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(v_head_1156_);
return v___x_1157_;
}
else
{
lean_object* v_head_1158_; lean_object* v___x_1159_; lean_object* v___x_1160_; 
lean_inc(v_tail_1155_);
v_head_1158_ = lean_ctor_get(v_x_1152_, 0);
lean_inc(v_head_1158_);
lean_dec_ref_known(v_x_1152_, 2);
v___x_1159_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(v_head_1158_);
v___x_1160_ = lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1_spec__2(v_x_1153_, v___x_1159_, v_tail_1155_);
return v___x_1160_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0___redArg(lean_object* v_a_1161_){
_start:
{
if (lean_obj_tag(v_a_1161_) == 0)
{
lean_object* v___x_1162_; 
v___x_1162_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1));
return v___x_1162_;
}
else
{
lean_object* v___x_1163_; lean_object* v___x_1164_; lean_object* v___x_1165_; lean_object* v___x_1166_; lean_object* v___x_1167_; lean_object* v___x_1168_; lean_object* v___x_1169_; lean_object* v___x_1170_; uint8_t v___x_1171_; lean_object* v___x_1172_; 
v___x_1163_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_1164_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__1(v_a_1161_, v___x_1163_);
v___x_1165_ = lean_obj_once(&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8, &lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8_once, _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8);
v___x_1166_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9));
v___x_1167_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1167_, 0, v___x_1166_);
lean_ctor_set(v___x_1167_, 1, v___x_1164_);
v___x_1168_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10));
v___x_1169_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1169_, 0, v___x_1167_);
lean_ctor_set(v___x_1169_, 1, v___x_1168_);
v___x_1170_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1170_, 0, v___x_1165_);
lean_ctor_set(v___x_1170_, 1, v___x_1169_);
v___x_1171_ = 0;
v___x_1172_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1172_, 0, v___x_1170_);
lean_ctor_set_uint8(v___x_1172_, sizeof(void*)*1, v___x_1171_);
return v___x_1172_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr(lean_object* v_x_1203_, lean_object* v_prec_1204_){
_start:
{
lean_object* v___y_1206_; lean_object* v___y_1213_; 
switch(lean_obj_tag(v_x_1203_))
{
case 0:
{
lean_object* v_b_1219_; lean_object* v___x_1221_; uint8_t v_isShared_1222_; uint8_t v_isSharedCheck_1239_; 
v_b_1219_ = lean_ctor_get(v_x_1203_, 0);
v_isSharedCheck_1239_ = !lean_is_exclusive(v_x_1203_);
if (v_isSharedCheck_1239_ == 0)
{
v___x_1221_ = v_x_1203_;
v_isShared_1222_ = v_isSharedCheck_1239_;
goto v_resetjp_1220_;
}
else
{
lean_inc(v_b_1219_);
lean_dec(v_x_1203_);
v___x_1221_ = lean_box(0);
v_isShared_1222_ = v_isSharedCheck_1239_;
goto v_resetjp_1220_;
}
v_resetjp_1220_:
{
lean_object* v___y_1224_; lean_object* v___x_1235_; uint8_t v___x_1236_; 
v___x_1235_ = lean_unsigned_to_nat(1024u);
v___x_1236_ = lean_nat_dec_le(v___x_1235_, v_prec_1204_);
if (v___x_1236_ == 0)
{
lean_object* v___x_1237_; 
v___x_1237_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_1224_ = v___x_1237_;
goto v___jp_1223_;
}
else
{
lean_object* v___x_1238_; 
v___x_1238_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_1224_ = v___x_1238_;
goto v___jp_1223_;
}
v___jp_1223_:
{
lean_object* v___x_1225_; lean_object* v___x_1226_; lean_object* v___x_1228_; 
v___x_1225_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__6));
v___x_1226_ = l_Nat_reprFast(v_b_1219_);
if (v_isShared_1222_ == 0)
{
lean_ctor_set_tag(v___x_1221_, 3);
lean_ctor_set(v___x_1221_, 0, v___x_1226_);
v___x_1228_ = v___x_1221_;
goto v_reusejp_1227_;
}
else
{
lean_object* v_reuseFailAlloc_1234_; 
v_reuseFailAlloc_1234_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v_reuseFailAlloc_1234_, 0, v___x_1226_);
v___x_1228_ = v_reuseFailAlloc_1234_;
goto v_reusejp_1227_;
}
v_reusejp_1227_:
{
lean_object* v___x_1229_; lean_object* v___x_1230_; uint8_t v___x_1231_; lean_object* v___x_1232_; lean_object* v___x_1233_; 
v___x_1229_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1229_, 0, v___x_1225_);
lean_ctor_set(v___x_1229_, 1, v___x_1228_);
lean_inc(v___y_1224_);
v___x_1230_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1230_, 0, v___y_1224_);
lean_ctor_set(v___x_1230_, 1, v___x_1229_);
v___x_1231_ = 0;
v___x_1232_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1232_, 0, v___x_1230_);
lean_ctor_set_uint8(v___x_1232_, sizeof(void*)*1, v___x_1231_);
v___x_1233_ = l_Repr_addAppParen(v___x_1232_, v_prec_1204_);
return v___x_1233_;
}
}
}
}
case 1:
{
lean_object* v_l_1240_; lean_object* v_cases_1241_; lean_object* v_other_1242_; lean_object* v___y_1244_; lean_object* v___x_1261_; uint8_t v___x_1262_; 
v_l_1240_ = lean_ctor_get(v_x_1203_, 0);
lean_inc(v_l_1240_);
v_cases_1241_ = lean_ctor_get(v_x_1203_, 1);
lean_inc(v_cases_1241_);
v_other_1242_ = lean_ctor_get(v_x_1203_, 2);
lean_inc(v_other_1242_);
lean_dec_ref_known(v_x_1203_, 3);
v___x_1261_ = lean_unsigned_to_nat(1024u);
v___x_1262_ = lean_nat_dec_le(v___x_1261_, v_prec_1204_);
if (v___x_1262_ == 0)
{
lean_object* v___x_1263_; 
v___x_1263_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_1244_ = v___x_1263_;
goto v___jp_1243_;
}
else
{
lean_object* v___x_1264_; 
v___x_1264_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_1244_ = v___x_1264_;
goto v___jp_1243_;
}
v___jp_1243_:
{
lean_object* v___x_1245_; lean_object* v___x_1246_; lean_object* v___x_1247_; lean_object* v___x_1248_; lean_object* v___x_1249_; lean_object* v___x_1250_; lean_object* v___x_1251_; lean_object* v___x_1252_; lean_object* v___x_1253_; lean_object* v___x_1254_; lean_object* v___x_1255_; lean_object* v___x_1256_; lean_object* v___x_1257_; uint8_t v___x_1258_; lean_object* v___x_1259_; lean_object* v___x_1260_; 
v___x_1245_ = lean_box(1);
v___x_1246_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__9));
v___x_1247_ = l_Nat_reprFast(v_l_1240_);
v___x_1248_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1248_, 0, v___x_1247_);
v___x_1249_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1249_, 0, v___x_1246_);
lean_ctor_set(v___x_1249_, 1, v___x_1248_);
v___x_1250_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1250_, 0, v___x_1249_);
lean_ctor_set(v___x_1250_, 1, v___x_1245_);
v___x_1251_ = lean_unsigned_to_nat(1024u);
v___x_1252_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0___redArg(v_cases_1241_);
v___x_1253_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1253_, 0, v___x_1250_);
lean_ctor_set(v___x_1253_, 1, v___x_1252_);
v___x_1254_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1254_, 0, v___x_1253_);
lean_ctor_set(v___x_1254_, 1, v___x_1245_);
v___x_1255_ = l_Option_repr___at___00Lean_Meta_instReprConfig__1_repr_spec__0(v_other_1242_, v___x_1251_);
v___x_1256_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1256_, 0, v___x_1254_);
lean_ctor_set(v___x_1256_, 1, v___x_1255_);
lean_inc(v___y_1244_);
v___x_1257_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1257_, 0, v___y_1244_);
lean_ctor_set(v___x_1257_, 1, v___x_1256_);
v___x_1258_ = 0;
v___x_1259_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1259_, 0, v___x_1257_);
lean_ctor_set_uint8(v___x_1259_, sizeof(void*)*1, v___x_1258_);
v___x_1260_ = l_Repr_addAppParen(v___x_1259_, v_prec_1204_);
return v___x_1260_;
}
}
case 2:
{
lean_object* v_dsts_1265_; lean_object* v_b_1266_; lean_object* v_same_1267_; lean_object* v_post_1268_; lean_object* v___y_1270_; lean_object* v___x_1289_; uint8_t v___x_1290_; 
v_dsts_1265_ = lean_ctor_get(v_x_1203_, 0);
lean_inc(v_dsts_1265_);
v_b_1266_ = lean_ctor_get(v_x_1203_, 1);
lean_inc(v_b_1266_);
v_same_1267_ = lean_ctor_get(v_x_1203_, 2);
lean_inc(v_same_1267_);
v_post_1268_ = lean_ctor_get(v_x_1203_, 3);
lean_inc(v_post_1268_);
lean_dec_ref_known(v_x_1203_, 4);
v___x_1289_ = lean_unsigned_to_nat(1024u);
v___x_1290_ = lean_nat_dec_le(v___x_1289_, v_prec_1204_);
if (v___x_1290_ == 0)
{
lean_object* v___x_1291_; 
v___x_1291_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_1270_ = v___x_1291_;
goto v___jp_1269_;
}
else
{
lean_object* v___x_1292_; 
v___x_1292_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_1270_ = v___x_1292_;
goto v___jp_1269_;
}
v___jp_1269_:
{
lean_object* v___x_1271_; lean_object* v___x_1272_; lean_object* v___x_1273_; lean_object* v___x_1274_; lean_object* v___x_1275_; lean_object* v___x_1276_; lean_object* v___x_1277_; lean_object* v___x_1278_; lean_object* v___x_1279_; lean_object* v___x_1280_; lean_object* v___x_1281_; lean_object* v___x_1282_; lean_object* v___x_1283_; lean_object* v___x_1284_; lean_object* v___x_1285_; uint8_t v___x_1286_; lean_object* v___x_1287_; lean_object* v___x_1288_; 
v___x_1271_ = lean_box(1);
v___x_1272_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__12));
v___x_1273_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_dsts_1265_);
v___x_1274_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1274_, 0, v___x_1272_);
lean_ctor_set(v___x_1274_, 1, v___x_1273_);
v___x_1275_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1275_, 0, v___x_1274_);
lean_ctor_set(v___x_1275_, 1, v___x_1271_);
v___x_1276_ = l_Nat_reprFast(v_b_1266_);
v___x_1277_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1277_, 0, v___x_1276_);
v___x_1278_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1278_, 0, v___x_1275_);
lean_ctor_set(v___x_1278_, 1, v___x_1277_);
v___x_1279_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1279_, 0, v___x_1278_);
lean_ctor_set(v___x_1279_, 1, v___x_1271_);
v___x_1280_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_same_1267_);
v___x_1281_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1281_, 0, v___x_1279_);
lean_ctor_set(v___x_1281_, 1, v___x_1280_);
v___x_1282_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1282_, 0, v___x_1281_);
lean_ctor_set(v___x_1282_, 1, v___x_1271_);
v___x_1283_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___redArg(v_post_1268_);
v___x_1284_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1284_, 0, v___x_1282_);
lean_ctor_set(v___x_1284_, 1, v___x_1283_);
lean_inc(v___y_1270_);
v___x_1285_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1285_, 0, v___y_1270_);
lean_ctor_set(v___x_1285_, 1, v___x_1284_);
v___x_1286_ = 0;
v___x_1287_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1287_, 0, v___x_1285_);
lean_ctor_set_uint8(v___x_1287_, sizeof(void*)*1, v___x_1286_);
v___x_1288_ = l_Repr_addAppParen(v___x_1287_, v_prec_1204_);
return v___x_1288_;
}
}
case 3:
{
lean_object* v_dst_1293_; lean_object* v_src_1294_; lean_object* v_pd_1295_; lean_object* v_ps_1296_; lean_object* v_b_1297_; lean_object* v___y_1299_; lean_object* v___x_1325_; uint8_t v___x_1326_; 
v_dst_1293_ = lean_ctor_get(v_x_1203_, 0);
lean_inc(v_dst_1293_);
v_src_1294_ = lean_ctor_get(v_x_1203_, 1);
lean_inc(v_src_1294_);
v_pd_1295_ = lean_ctor_get(v_x_1203_, 2);
lean_inc(v_pd_1295_);
v_ps_1296_ = lean_ctor_get(v_x_1203_, 3);
lean_inc(v_ps_1296_);
v_b_1297_ = lean_ctor_get(v_x_1203_, 4);
lean_inc(v_b_1297_);
lean_dec_ref_known(v_x_1203_, 5);
v___x_1325_ = lean_unsigned_to_nat(1024u);
v___x_1326_ = lean_nat_dec_le(v___x_1325_, v_prec_1204_);
if (v___x_1326_ == 0)
{
lean_object* v___x_1327_; 
v___x_1327_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_1299_ = v___x_1327_;
goto v___jp_1298_;
}
else
{
lean_object* v___x_1328_; 
v___x_1328_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_1299_ = v___x_1328_;
goto v___jp_1298_;
}
v___jp_1298_:
{
lean_object* v___x_1300_; lean_object* v___x_1301_; lean_object* v___x_1302_; lean_object* v___x_1303_; lean_object* v___x_1304_; lean_object* v___x_1305_; lean_object* v___x_1306_; lean_object* v___x_1307_; lean_object* v___x_1308_; lean_object* v___x_1309_; lean_object* v___x_1310_; lean_object* v___x_1311_; lean_object* v___x_1312_; lean_object* v___x_1313_; lean_object* v___x_1314_; lean_object* v___x_1315_; lean_object* v___x_1316_; lean_object* v___x_1317_; lean_object* v___x_1318_; lean_object* v___x_1319_; lean_object* v___x_1320_; lean_object* v___x_1321_; uint8_t v___x_1322_; lean_object* v___x_1323_; lean_object* v___x_1324_; 
v___x_1300_ = lean_box(1);
v___x_1301_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__15));
v___x_1302_ = l_Nat_reprFast(v_dst_1293_);
v___x_1303_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1303_, 0, v___x_1302_);
v___x_1304_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1304_, 0, v___x_1301_);
lean_ctor_set(v___x_1304_, 1, v___x_1303_);
v___x_1305_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1305_, 0, v___x_1304_);
lean_ctor_set(v___x_1305_, 1, v___x_1300_);
v___x_1306_ = l_Nat_reprFast(v_src_1294_);
v___x_1307_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1307_, 0, v___x_1306_);
v___x_1308_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1308_, 0, v___x_1305_);
lean_ctor_set(v___x_1308_, 1, v___x_1307_);
v___x_1309_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1309_, 0, v___x_1308_);
lean_ctor_set(v___x_1309_, 1, v___x_1300_);
v___x_1310_ = l_Nat_reprFast(v_pd_1295_);
v___x_1311_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1311_, 0, v___x_1310_);
v___x_1312_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1312_, 0, v___x_1309_);
lean_ctor_set(v___x_1312_, 1, v___x_1311_);
v___x_1313_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1313_, 0, v___x_1312_);
lean_ctor_set(v___x_1313_, 1, v___x_1300_);
v___x_1314_ = l_Nat_reprFast(v_ps_1296_);
v___x_1315_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1315_, 0, v___x_1314_);
v___x_1316_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1316_, 0, v___x_1313_);
lean_ctor_set(v___x_1316_, 1, v___x_1315_);
v___x_1317_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1317_, 0, v___x_1316_);
lean_ctor_set(v___x_1317_, 1, v___x_1300_);
v___x_1318_ = l_Nat_reprFast(v_b_1297_);
v___x_1319_ = lean_alloc_ctor(3, 1, 0);
lean_ctor_set(v___x_1319_, 0, v___x_1318_);
v___x_1320_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1320_, 0, v___x_1317_);
lean_ctor_set(v___x_1320_, 1, v___x_1319_);
lean_inc(v___y_1299_);
v___x_1321_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1321_, 0, v___y_1299_);
lean_ctor_set(v___x_1321_, 1, v___x_1320_);
v___x_1322_ = 0;
v___x_1323_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1323_, 0, v___x_1321_);
lean_ctor_set_uint8(v___x_1323_, sizeof(void*)*1, v___x_1322_);
v___x_1324_ = l_Repr_addAppParen(v___x_1323_, v_prec_1204_);
return v___x_1324_;
}
}
case 4:
{
lean_object* v___x_1329_; uint8_t v___x_1330_; 
v___x_1329_ = lean_unsigned_to_nat(1024u);
v___x_1330_ = lean_nat_dec_le(v___x_1329_, v_prec_1204_);
if (v___x_1330_ == 0)
{
lean_object* v___x_1331_; 
v___x_1331_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_1206_ = v___x_1331_;
goto v___jp_1205_;
}
else
{
lean_object* v___x_1332_; 
v___x_1332_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_1206_ = v___x_1332_;
goto v___jp_1205_;
}
}
default: 
{
lean_object* v___x_1333_; uint8_t v___x_1334_; 
v___x_1333_ = lean_unsigned_to_nat(1024u);
v___x_1334_ = lean_nat_dec_le(v___x_1333_, v_prec_1204_);
if (v___x_1334_ == 0)
{
lean_object* v___x_1335_; 
v___x_1335_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__3);
v___y_1213_ = v___x_1335_;
goto v___jp_1212_;
}
else
{
lean_object* v___x_1336_; 
v___x_1336_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4, &lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprTest_repr___closed__4);
v___y_1213_ = v___x_1336_;
goto v___jp_1212_;
}
}
}
v___jp_1205_:
{
lean_object* v___x_1207_; lean_object* v___x_1208_; uint8_t v___x_1209_; lean_object* v___x_1210_; lean_object* v___x_1211_; 
v___x_1207_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__1));
lean_inc(v___y_1206_);
v___x_1208_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1208_, 0, v___y_1206_);
lean_ctor_set(v___x_1208_, 1, v___x_1207_);
v___x_1209_ = 0;
v___x_1210_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1210_, 0, v___x_1208_);
lean_ctor_set_uint8(v___x_1210_, sizeof(void*)*1, v___x_1209_);
v___x_1211_ = l_Repr_addAppParen(v___x_1210_, v_prec_1204_);
return v___x_1211_;
}
v___jp_1212_:
{
lean_object* v___x_1214_; lean_object* v___x_1215_; uint8_t v___x_1216_; lean_object* v___x_1217_; lean_object* v___x_1218_; 
v___x_1214_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprTerm_repr___closed__3));
lean_inc(v___y_1213_);
v___x_1215_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1215_, 0, v___y_1213_);
lean_ctor_set(v___x_1215_, 1, v___x_1214_);
v___x_1216_ = 0;
v___x_1217_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1217_, 0, v___x_1215_);
lean_ctor_set_uint8(v___x_1217_, sizeof(void*)*1, v___x_1216_);
v___x_1218_ = l_Repr_addAppParen(v___x_1217_, v_prec_1204_);
return v___x_1218_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprTerm_repr___boxed(lean_object* v_x_1337_, lean_object* v_prec_1338_){
_start:
{
lean_object* v_res_1339_; 
v_res_1339_ = lp_clauseflow_ClauseFlow_instReprTerm_repr(v_x_1337_, v_prec_1338_);
lean_dec(v_prec_1338_);
return v_res_1339_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0(lean_object* v_a_1340_, lean_object* v_n_1341_){
_start:
{
lean_object* v___x_1342_; 
v___x_1342_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0___redArg(v_a_1340_);
return v___x_1342_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0___boxed(lean_object* v_a_1343_, lean_object* v_n_1344_){
_start:
{
lean_object* v_res_1345_; 
v_res_1345_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0(v_a_1343_, v_n_1344_);
lean_dec(v_n_1344_);
return v_res_1345_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1(lean_object* v_a_1346_, lean_object* v_n_1347_){
_start:
{
lean_object* v___x_1348_; 
v___x_1348_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___redArg(v_a_1346_);
return v___x_1348_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___boxed(lean_object* v_a_1349_, lean_object* v_n_1350_){
_start:
{
lean_object* v_res_1351_; 
v_res_1351_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1(v_a_1349_, v_n_1350_);
lean_dec(v_n_1350_);
return v_res_1351_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0(lean_object* v_x_1352_, lean_object* v_x_1353_){
_start:
{
lean_object* v___x_1354_; 
v___x_1354_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___redArg(v_x_1352_);
return v___x_1354_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0___boxed(lean_object* v_x_1355_, lean_object* v_x_1356_){
_start:
{
lean_object* v_res_1357_; 
v_res_1357_ = lp_clauseflow_Prod_repr___at___00List_repr___at___00ClauseFlow_instReprTerm_repr_spec__0_spec__0(v_x_1355_, v_x_1356_);
lean_dec(v_x_1356_);
return v_res_1357_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0___lam__0(lean_object* v___y_1360_){
_start:
{
lean_object* v___x_1361_; lean_object* v___x_1362_; 
v___x_1361_ = lean_unsigned_to_nat(0u);
v___x_1362_ = lp_clauseflow_ClauseFlow_instReprStmt_repr(v___y_1360_, v___x_1361_);
return v___x_1362_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0_spec__1_spec__2(lean_object* v_x_1363_, lean_object* v_x_1364_, lean_object* v_x_1365_){
_start:
{
if (lean_obj_tag(v_x_1365_) == 0)
{
lean_dec(v_x_1363_);
return v_x_1364_;
}
else
{
lean_object* v_head_1366_; lean_object* v_tail_1367_; lean_object* v___x_1369_; uint8_t v_isShared_1370_; uint8_t v_isSharedCheck_1378_; 
v_head_1366_ = lean_ctor_get(v_x_1365_, 0);
v_tail_1367_ = lean_ctor_get(v_x_1365_, 1);
v_isSharedCheck_1378_ = !lean_is_exclusive(v_x_1365_);
if (v_isSharedCheck_1378_ == 0)
{
v___x_1369_ = v_x_1365_;
v_isShared_1370_ = v_isSharedCheck_1378_;
goto v_resetjp_1368_;
}
else
{
lean_inc(v_tail_1367_);
lean_inc(v_head_1366_);
lean_dec(v_x_1365_);
v___x_1369_ = lean_box(0);
v_isShared_1370_ = v_isSharedCheck_1378_;
goto v_resetjp_1368_;
}
v_resetjp_1368_:
{
lean_object* v___x_1372_; 
lean_inc(v_x_1363_);
if (v_isShared_1370_ == 0)
{
lean_ctor_set_tag(v___x_1369_, 5);
lean_ctor_set(v___x_1369_, 1, v_x_1363_);
lean_ctor_set(v___x_1369_, 0, v_x_1364_);
v___x_1372_ = v___x_1369_;
goto v_reusejp_1371_;
}
else
{
lean_object* v_reuseFailAlloc_1377_; 
v_reuseFailAlloc_1377_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1377_, 0, v_x_1364_);
lean_ctor_set(v_reuseFailAlloc_1377_, 1, v_x_1363_);
v___x_1372_ = v_reuseFailAlloc_1377_;
goto v_reusejp_1371_;
}
v_reusejp_1371_:
{
lean_object* v___x_1373_; lean_object* v___x_1374_; lean_object* v___x_1375_; 
v___x_1373_ = lean_unsigned_to_nat(0u);
v___x_1374_ = lp_clauseflow_ClauseFlow_instReprStmt_repr(v_head_1366_, v___x_1373_);
v___x_1375_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1375_, 0, v___x_1372_);
lean_ctor_set(v___x_1375_, 1, v___x_1374_);
v_x_1364_ = v___x_1375_;
v_x_1365_ = v_tail_1367_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0_spec__1(lean_object* v_x_1379_, lean_object* v_x_1380_, lean_object* v_x_1381_){
_start:
{
if (lean_obj_tag(v_x_1381_) == 0)
{
lean_dec(v_x_1379_);
return v_x_1380_;
}
else
{
lean_object* v_head_1382_; lean_object* v_tail_1383_; lean_object* v___x_1385_; uint8_t v_isShared_1386_; uint8_t v_isSharedCheck_1394_; 
v_head_1382_ = lean_ctor_get(v_x_1381_, 0);
v_tail_1383_ = lean_ctor_get(v_x_1381_, 1);
v_isSharedCheck_1394_ = !lean_is_exclusive(v_x_1381_);
if (v_isSharedCheck_1394_ == 0)
{
v___x_1385_ = v_x_1381_;
v_isShared_1386_ = v_isSharedCheck_1394_;
goto v_resetjp_1384_;
}
else
{
lean_inc(v_tail_1383_);
lean_inc(v_head_1382_);
lean_dec(v_x_1381_);
v___x_1385_ = lean_box(0);
v_isShared_1386_ = v_isSharedCheck_1394_;
goto v_resetjp_1384_;
}
v_resetjp_1384_:
{
lean_object* v___x_1388_; 
lean_inc(v_x_1379_);
if (v_isShared_1386_ == 0)
{
lean_ctor_set_tag(v___x_1385_, 5);
lean_ctor_set(v___x_1385_, 1, v_x_1379_);
lean_ctor_set(v___x_1385_, 0, v_x_1380_);
v___x_1388_ = v___x_1385_;
goto v_reusejp_1387_;
}
else
{
lean_object* v_reuseFailAlloc_1393_; 
v_reuseFailAlloc_1393_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1393_, 0, v_x_1380_);
lean_ctor_set(v_reuseFailAlloc_1393_, 1, v_x_1379_);
v___x_1388_ = v_reuseFailAlloc_1393_;
goto v_reusejp_1387_;
}
v_reusejp_1387_:
{
lean_object* v___x_1389_; lean_object* v___x_1390_; lean_object* v___x_1391_; lean_object* v___x_1392_; 
v___x_1389_ = lean_unsigned_to_nat(0u);
v___x_1390_ = lp_clauseflow_ClauseFlow_instReprStmt_repr(v_head_1382_, v___x_1389_);
v___x_1391_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1391_, 0, v___x_1388_);
lean_ctor_set(v___x_1391_, 1, v___x_1390_);
v___x_1392_ = lp_clauseflow_List_foldl___at___00List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0_spec__1_spec__2(v_x_1379_, v___x_1391_, v_tail_1383_);
return v___x_1392_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0(lean_object* v_x_1395_, lean_object* v_x_1396_){
_start:
{
if (lean_obj_tag(v_x_1395_) == 0)
{
lean_object* v___x_1397_; 
lean_dec(v_x_1396_);
v___x_1397_ = lean_box(0);
return v___x_1397_;
}
else
{
lean_object* v_tail_1398_; 
v_tail_1398_ = lean_ctor_get(v_x_1395_, 1);
if (lean_obj_tag(v_tail_1398_) == 0)
{
lean_object* v_head_1399_; lean_object* v___x_1400_; 
lean_dec(v_x_1396_);
v_head_1399_ = lean_ctor_get(v_x_1395_, 0);
lean_inc(v_head_1399_);
lean_dec_ref_known(v_x_1395_, 2);
v___x_1400_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0___lam__0(v_head_1399_);
return v___x_1400_;
}
else
{
lean_object* v_head_1401_; lean_object* v___x_1402_; lean_object* v___x_1403_; 
lean_inc(v_tail_1398_);
v_head_1401_ = lean_ctor_get(v_x_1395_, 0);
lean_inc(v_head_1401_);
lean_dec_ref_known(v_x_1395_, 2);
v___x_1402_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0___lam__0(v_head_1401_);
v___x_1403_ = lp_clauseflow_List_foldl___at___00Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0_spec__1(v_x_1396_, v___x_1402_, v_tail_1398_);
return v___x_1403_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0___redArg(lean_object* v_a_1404_){
_start:
{
if (lean_obj_tag(v_a_1404_) == 0)
{
lean_object* v___x_1405_; 
v___x_1405_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__1));
return v___x_1405_;
}
else
{
lean_object* v___x_1406_; lean_object* v___x_1407_; lean_object* v___x_1408_; lean_object* v___x_1409_; lean_object* v___x_1410_; lean_object* v___x_1411_; lean_object* v___x_1412_; lean_object* v___x_1413_; uint8_t v___x_1414_; lean_object* v___x_1415_; 
v___x_1406_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__5));
v___x_1407_ = lp_clauseflow_Std_Format_joinSep___at___00List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0_spec__0(v_a_1404_, v___x_1406_);
v___x_1408_ = lean_obj_once(&lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8, &lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8_once, _init_lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__8);
v___x_1409_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__9));
v___x_1410_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1410_, 0, v___x_1409_);
lean_ctor_set(v___x_1410_, 1, v___x_1407_);
v___x_1411_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__10));
v___x_1412_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1412_, 0, v___x_1410_);
lean_ctor_set(v___x_1412_, 1, v___x_1411_);
v___x_1413_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1413_, 0, v___x_1408_);
lean_ctor_set(v___x_1413_, 1, v___x_1412_);
v___x_1414_ = 0;
v___x_1415_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1415_, 0, v___x_1413_);
lean_ctor_set_uint8(v___x_1415_, sizeof(void*)*1, v___x_1414_);
return v___x_1415_;
}
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4(void){
_start:
{
lean_object* v___x_1425_; lean_object* v___x_1426_; 
v___x_1425_ = lean_unsigned_to_nat(9u);
v___x_1426_ = lean_nat_to_int(v___x_1425_);
return v___x_1426_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7(void){
_start:
{
lean_object* v___x_1430_; lean_object* v___x_1431_; 
v___x_1430_ = lean_unsigned_to_nat(8u);
v___x_1431_ = lean_nat_to_int(v___x_1430_);
return v___x_1431_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg(lean_object* v_x_1432_){
_start:
{
lean_object* v_stmts_1433_; lean_object* v_term_1434_; lean_object* v___x_1436_; uint8_t v_isShared_1437_; uint8_t v_isSharedCheck_1468_; 
v_stmts_1433_ = lean_ctor_get(v_x_1432_, 0);
v_term_1434_ = lean_ctor_get(v_x_1432_, 1);
v_isSharedCheck_1468_ = !lean_is_exclusive(v_x_1432_);
if (v_isSharedCheck_1468_ == 0)
{
v___x_1436_ = v_x_1432_;
v_isShared_1437_ = v_isSharedCheck_1468_;
goto v_resetjp_1435_;
}
else
{
lean_inc(v_term_1434_);
lean_inc(v_stmts_1433_);
lean_dec(v_x_1432_);
v___x_1436_ = lean_box(0);
v_isShared_1437_ = v_isSharedCheck_1468_;
goto v_resetjp_1435_;
}
v_resetjp_1435_:
{
lean_object* v___x_1438_; lean_object* v___x_1439_; lean_object* v___x_1440_; lean_object* v___x_1441_; lean_object* v___x_1442_; lean_object* v___x_1444_; 
v___x_1438_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5));
v___x_1439_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__3));
v___x_1440_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4, &lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4);
v___x_1441_ = lean_unsigned_to_nat(0u);
v___x_1442_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0___redArg(v_stmts_1433_);
if (v_isShared_1437_ == 0)
{
lean_ctor_set_tag(v___x_1436_, 4);
lean_ctor_set(v___x_1436_, 1, v___x_1442_);
lean_ctor_set(v___x_1436_, 0, v___x_1440_);
v___x_1444_ = v___x_1436_;
goto v_reusejp_1443_;
}
else
{
lean_object* v_reuseFailAlloc_1467_; 
v_reuseFailAlloc_1467_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1467_, 0, v___x_1440_);
lean_ctor_set(v_reuseFailAlloc_1467_, 1, v___x_1442_);
v___x_1444_ = v_reuseFailAlloc_1467_;
goto v_reusejp_1443_;
}
v_reusejp_1443_:
{
uint8_t v___x_1445_; lean_object* v___x_1446_; lean_object* v___x_1447_; lean_object* v___x_1448_; lean_object* v___x_1449_; lean_object* v___x_1450_; lean_object* v___x_1451_; lean_object* v___x_1452_; lean_object* v___x_1453_; lean_object* v___x_1454_; lean_object* v___x_1455_; lean_object* v___x_1456_; lean_object* v___x_1457_; lean_object* v___x_1458_; lean_object* v___x_1459_; lean_object* v___x_1460_; lean_object* v___x_1461_; lean_object* v___x_1462_; lean_object* v___x_1463_; lean_object* v___x_1464_; lean_object* v___x_1465_; lean_object* v___x_1466_; 
v___x_1445_ = 0;
v___x_1446_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1446_, 0, v___x_1444_);
lean_ctor_set_uint8(v___x_1446_, sizeof(void*)*1, v___x_1445_);
v___x_1447_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1447_, 0, v___x_1439_);
lean_ctor_set(v___x_1447_, 1, v___x_1446_);
v___x_1448_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4));
v___x_1449_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1449_, 0, v___x_1447_);
lean_ctor_set(v___x_1449_, 1, v___x_1448_);
v___x_1450_ = lean_box(1);
v___x_1451_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1451_, 0, v___x_1449_);
lean_ctor_set(v___x_1451_, 1, v___x_1450_);
v___x_1452_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__6));
v___x_1453_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1453_, 0, v___x_1451_);
lean_ctor_set(v___x_1453_, 1, v___x_1452_);
v___x_1454_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1454_, 0, v___x_1453_);
lean_ctor_set(v___x_1454_, 1, v___x_1438_);
v___x_1455_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7, &lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7_once, _init_lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7);
v___x_1456_ = lp_clauseflow_ClauseFlow_instReprTerm_repr(v_term_1434_, v___x_1441_);
v___x_1457_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1457_, 0, v___x_1455_);
lean_ctor_set(v___x_1457_, 1, v___x_1456_);
v___x_1458_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1458_, 0, v___x_1457_);
lean_ctor_set_uint8(v___x_1458_, sizeof(void*)*1, v___x_1445_);
v___x_1459_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1459_, 0, v___x_1454_);
lean_ctor_set(v___x_1459_, 1, v___x_1458_);
v___x_1460_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13);
v___x_1461_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14));
v___x_1462_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1462_, 0, v___x_1461_);
lean_ctor_set(v___x_1462_, 1, v___x_1459_);
v___x_1463_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15));
v___x_1464_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1464_, 0, v___x_1462_);
lean_ctor_set(v___x_1464_, 1, v___x_1463_);
v___x_1465_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1465_, 0, v___x_1460_);
lean_ctor_set(v___x_1465_, 1, v___x_1464_);
v___x_1466_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1466_, 0, v___x_1465_);
lean_ctor_set_uint8(v___x_1466_, sizeof(void*)*1, v___x_1445_);
return v___x_1466_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr(lean_object* v_x_1469_, lean_object* v_prec_1470_){
_start:
{
lean_object* v___x_1471_; 
v___x_1471_ = lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg(v_x_1469_);
return v___x_1471_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprBlockD_repr___boxed(lean_object* v_x_1472_, lean_object* v_prec_1473_){
_start:
{
lean_object* v_res_1474_; 
v_res_1474_ = lp_clauseflow_ClauseFlow_instReprBlockD_repr(v_x_1472_, v_prec_1473_);
lean_dec(v_prec_1473_);
return v_res_1474_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0(lean_object* v_a_1475_, lean_object* v_n_1476_){
_start:
{
lean_object* v___x_1477_; 
v___x_1477_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0___redArg(v_a_1475_);
return v___x_1477_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0___boxed(lean_object* v_a_1478_, lean_object* v_n_1479_){
_start:
{
lean_object* v_res_1480_; 
v_res_1480_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprBlockD_repr_spec__0(v_a_1478_, v_n_1479_);
lean_dec(v_n_1479_);
return v_res_1480_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_upd(lean_object* v_env_1483_, lean_object* v_l_1484_, lean_object* v_v_1485_, lean_object* v_x_1486_){
_start:
{
uint8_t v___x_1487_; 
v___x_1487_ = lean_nat_dec_eq(v_x_1486_, v_l_1484_);
if (v___x_1487_ == 0)
{
lean_object* v___x_1488_; 
v___x_1488_ = lean_apply_1(v_env_1483_, v_x_1486_);
return v___x_1488_;
}
else
{
lean_dec(v_x_1486_);
lean_dec_ref(v_env_1483_);
lean_inc(v_v_1485_);
return v_v_1485_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_upd___boxed(lean_object* v_env_1489_, lean_object* v_l_1490_, lean_object* v_v_1491_, lean_object* v_x_1492_){
_start:
{
lean_object* v_res_1493_; 
v_res_1493_ = lp_clauseflow_ClauseFlow_upd(v_env_1489_, v_l_1490_, v_v_1491_, v_x_1492_);
lean_dec(v_v_1491_);
lean_dec(v_l_1490_);
return v_res_1493_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_lookupCase(lean_object* v_x_1494_, lean_object* v_x_1495_){
_start:
{
if (lean_obj_tag(v_x_1494_) == 0)
{
lean_object* v___x_1496_; 
v___x_1496_ = lean_box(0);
return v___x_1496_;
}
else
{
lean_object* v_head_1497_; lean_object* v_tail_1498_; lean_object* v_fst_1499_; lean_object* v_snd_1500_; uint8_t v___x_1501_; 
v_head_1497_ = lean_ctor_get(v_x_1494_, 0);
v_tail_1498_ = lean_ctor_get(v_x_1494_, 1);
v_fst_1499_ = lean_ctor_get(v_head_1497_, 0);
v_snd_1500_ = lean_ctor_get(v_head_1497_, 1);
v___x_1501_ = lean_nat_dec_eq(v_fst_1499_, v_x_1495_);
if (v___x_1501_ == 0)
{
v_x_1494_ = v_tail_1498_;
goto _start;
}
else
{
lean_object* v___x_1503_; 
lean_inc(v_snd_1500_);
v___x_1503_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_1503_, 0, v_snd_1500_);
return v___x_1503_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_lookupCase___boxed(lean_object* v_x_1504_, lean_object* v_x_1505_){
_start:
{
lean_object* v_res_1506_; 
v_res_1506_ = lp_clauseflow_ClauseFlow_lookupCase(v_x_1504_, v_x_1505_);
lean_dec(v_x_1505_);
lean_dec(v_x_1504_);
return v_res_1506_;
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6(void){
_start:
{
lean_object* v___x_1519_; lean_object* v___x_1520_; 
v___x_1519_ = lean_unsigned_to_nat(7u);
v___x_1520_ = lean_nat_to_int(v___x_1519_);
return v___x_1520_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___redArg(lean_object* v_x_1521_){
_start:
{
lean_object* v_facts_1522_; lean_object* v_cls_1523_; lean_object* v___x_1525_; uint8_t v_isShared_1526_; uint8_t v_isSharedCheck_1556_; 
v_facts_1522_ = lean_ctor_get(v_x_1521_, 0);
v_cls_1523_ = lean_ctor_get(v_x_1521_, 1);
v_isSharedCheck_1556_ = !lean_is_exclusive(v_x_1521_);
if (v_isSharedCheck_1556_ == 0)
{
v___x_1525_ = v_x_1521_;
v_isShared_1526_ = v_isSharedCheck_1556_;
goto v_resetjp_1524_;
}
else
{
lean_inc(v_cls_1523_);
lean_inc(v_facts_1522_);
lean_dec(v_x_1521_);
v___x_1525_ = lean_box(0);
v_isShared_1526_ = v_isSharedCheck_1556_;
goto v_resetjp_1524_;
}
v_resetjp_1524_:
{
lean_object* v___x_1527_; lean_object* v___x_1528_; lean_object* v___x_1529_; lean_object* v___x_1530_; lean_object* v___x_1532_; 
v___x_1527_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5));
v___x_1528_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__3));
v___x_1529_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4, &lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__4);
v___x_1530_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprClause_repr_spec__0___redArg(v_facts_1522_);
if (v_isShared_1526_ == 0)
{
lean_ctor_set_tag(v___x_1525_, 4);
lean_ctor_set(v___x_1525_, 1, v___x_1530_);
lean_ctor_set(v___x_1525_, 0, v___x_1529_);
v___x_1532_ = v___x_1525_;
goto v_reusejp_1531_;
}
else
{
lean_object* v_reuseFailAlloc_1555_; 
v_reuseFailAlloc_1555_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1555_, 0, v___x_1529_);
lean_ctor_set(v_reuseFailAlloc_1555_, 1, v___x_1530_);
v___x_1532_ = v_reuseFailAlloc_1555_;
goto v_reusejp_1531_;
}
v_reusejp_1531_:
{
uint8_t v___x_1533_; lean_object* v___x_1534_; lean_object* v___x_1535_; lean_object* v___x_1536_; lean_object* v___x_1537_; lean_object* v___x_1538_; lean_object* v___x_1539_; lean_object* v___x_1540_; lean_object* v___x_1541_; lean_object* v___x_1542_; lean_object* v___x_1543_; lean_object* v___x_1544_; lean_object* v___x_1545_; lean_object* v___x_1546_; lean_object* v___x_1547_; lean_object* v___x_1548_; lean_object* v___x_1549_; lean_object* v___x_1550_; lean_object* v___x_1551_; lean_object* v___x_1552_; lean_object* v___x_1553_; lean_object* v___x_1554_; 
v___x_1533_ = 0;
v___x_1534_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1534_, 0, v___x_1532_);
lean_ctor_set_uint8(v___x_1534_, sizeof(void*)*1, v___x_1533_);
v___x_1535_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1535_, 0, v___x_1528_);
lean_ctor_set(v___x_1535_, 1, v___x_1534_);
v___x_1536_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4));
v___x_1537_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1537_, 0, v___x_1535_);
lean_ctor_set(v___x_1537_, 1, v___x_1536_);
v___x_1538_ = lean_box(1);
v___x_1539_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1539_, 0, v___x_1537_);
lean_ctor_set(v___x_1539_, 1, v___x_1538_);
v___x_1540_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__5));
v___x_1541_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1541_, 0, v___x_1539_);
lean_ctor_set(v___x_1541_, 1, v___x_1540_);
v___x_1542_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1542_, 0, v___x_1541_);
lean_ctor_set(v___x_1542_, 1, v___x_1527_);
v___x_1543_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6, &lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6_once, _init_lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6);
v___x_1544_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___redArg(v_cls_1523_);
v___x_1545_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1545_, 0, v___x_1543_);
lean_ctor_set(v___x_1545_, 1, v___x_1544_);
v___x_1546_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1546_, 0, v___x_1545_);
lean_ctor_set_uint8(v___x_1546_, sizeof(void*)*1, v___x_1533_);
v___x_1547_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1547_, 0, v___x_1542_);
lean_ctor_set(v___x_1547_, 1, v___x_1546_);
v___x_1548_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13);
v___x_1549_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14));
v___x_1550_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1550_, 0, v___x_1549_);
lean_ctor_set(v___x_1550_, 1, v___x_1547_);
v___x_1551_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15));
v___x_1552_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_1552_, 0, v___x_1550_);
lean_ctor_set(v___x_1552_, 1, v___x_1551_);
v___x_1553_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_1553_, 0, v___x_1548_);
lean_ctor_set(v___x_1553_, 1, v___x_1552_);
v___x_1554_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_1554_, 0, v___x_1553_);
lean_ctor_set_uint8(v___x_1554_, sizeof(void*)*1, v___x_1533_);
return v___x_1554_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr(lean_object* v_x_1557_, lean_object* v_prec_1558_){
_start:
{
lean_object* v___x_1559_; 
v___x_1559_ = lp_clauseflow_ClauseFlow_instReprAState_repr___redArg(v_x_1557_);
return v___x_1559_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprAState_repr___boxed(lean_object* v_x_1560_, lean_object* v_prec_1561_){
_start:
{
lean_object* v_res_1562_; 
v_res_1562_ = lp_clauseflow_ClauseFlow_instReprAState_repr(v_x_1560_, v_prec_1561_);
lean_dec(v_prec_1561_);
return v_res_1562_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__0(lean_object* v_p_1565_, lean_object* v_x_1566_){
_start:
{
if (lean_obj_tag(v_x_1566_) == 0)
{
uint8_t v___x_1567_; 
v___x_1567_ = 0;
return v___x_1567_;
}
else
{
lean_object* v_head_1568_; lean_object* v_tail_1569_; uint8_t v___y_1571_; lean_object* v_fst_1573_; lean_object* v_snd_1574_; lean_object* v_fst_1575_; lean_object* v_snd_1576_; uint8_t v___x_1577_; 
v_head_1568_ = lean_ctor_get(v_x_1566_, 0);
v_tail_1569_ = lean_ctor_get(v_x_1566_, 1);
v_fst_1573_ = lean_ctor_get(v_p_1565_, 0);
v_snd_1574_ = lean_ctor_get(v_p_1565_, 1);
v_fst_1575_ = lean_ctor_get(v_head_1568_, 0);
v_snd_1576_ = lean_ctor_get(v_head_1568_, 1);
v___x_1577_ = lean_nat_dec_eq(v_fst_1573_, v_fst_1575_);
if (v___x_1577_ == 0)
{
v___y_1571_ = v___x_1577_;
goto v___jp_1570_;
}
else
{
uint8_t v___x_1578_; 
v___x_1578_ = lp_clauseflow_ClauseFlow_Test_conflict(v_snd_1574_, v_snd_1576_);
v___y_1571_ = v___x_1578_;
goto v___jp_1570_;
}
v___jp_1570_:
{
if (v___y_1571_ == 0)
{
v_x_1566_ = v_tail_1569_;
goto _start;
}
else
{
return v___y_1571_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__0___boxed(lean_object* v_p_1579_, lean_object* v_x_1580_){
_start:
{
uint8_t v_res_1581_; lean_object* v_r_1582_; 
v_res_1581_ = lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__0(v_p_1579_, v_x_1580_);
lean_dec(v_x_1580_);
lean_dec_ref(v_p_1579_);
v_r_1582_ = lean_box(v_res_1581_);
return v_r_1582_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1(lean_object* v_L_1583_, lean_object* v_x_1584_){
_start:
{
if (lean_obj_tag(v_x_1584_) == 0)
{
uint8_t v___x_1585_; 
v___x_1585_ = 0;
return v___x_1585_;
}
else
{
lean_object* v_head_1586_; lean_object* v_tail_1587_; uint8_t v___x_1588_; 
v_head_1586_ = lean_ctor_get(v_x_1584_, 0);
v_tail_1587_ = lean_ctor_get(v_x_1584_, 1);
v___x_1588_ = lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__0(v_head_1586_, v_L_1583_);
if (v___x_1588_ == 0)
{
v_x_1584_ = v_tail_1587_;
goto _start;
}
else
{
return v___x_1588_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1___boxed(lean_object* v_L_1590_, lean_object* v_x_1591_){
_start:
{
uint8_t v_res_1592_; lean_object* v_r_1593_; 
v_res_1592_ = lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1(v_L_1590_, v_x_1591_);
lean_dec(v_x_1591_);
lean_dec(v_L_1590_);
v_r_1593_ = lean_box(v_res_1592_);
return v_r_1593_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_inconsistent(lean_object* v_L_1594_){
_start:
{
uint8_t v___x_1595_; 
v___x_1595_ = lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1(v_L_1594_, v_L_1594_);
return v___x_1595_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_inconsistent___boxed(lean_object* v_L_1596_){
_start:
{
uint8_t v_res_1597_; lean_object* v_r_1598_; 
v_res_1597_ = lp_clauseflow_ClauseFlow_inconsistent(v_L_1596_);
lean_dec(v_L_1596_);
v_r_1598_ = lean_box(v_res_1597_);
return v_r_1598_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0(lean_object* v_g_1599_, lean_object* v_x_1600_){
_start:
{
if (lean_obj_tag(v_x_1600_) == 0)
{
uint8_t v___x_1601_; 
v___x_1601_ = 0;
return v___x_1601_;
}
else
{
lean_object* v_head_1602_; lean_object* v_tail_1603_; uint8_t v___y_1605_; lean_object* v_fst_1607_; lean_object* v_snd_1608_; lean_object* v_fst_1609_; lean_object* v_snd_1610_; uint8_t v___x_1611_; 
v_head_1602_ = lean_ctor_get(v_x_1600_, 0);
v_tail_1603_ = lean_ctor_get(v_x_1600_, 1);
v_fst_1607_ = lean_ctor_get(v_head_1602_, 0);
v_snd_1608_ = lean_ctor_get(v_head_1602_, 1);
v_fst_1609_ = lean_ctor_get(v_g_1599_, 0);
v_snd_1610_ = lean_ctor_get(v_g_1599_, 1);
v___x_1611_ = lean_nat_dec_eq(v_fst_1607_, v_fst_1609_);
if (v___x_1611_ == 0)
{
v___y_1605_ = v___x_1611_;
goto v___jp_1604_;
}
else
{
uint8_t v___x_1612_; 
v___x_1612_ = lp_clauseflow_ClauseFlow_Test_imp(v_snd_1608_, v_snd_1610_);
v___y_1605_ = v___x_1612_;
goto v___jp_1604_;
}
v___jp_1604_:
{
if (v___y_1605_ == 0)
{
v_x_1600_ = v_tail_1603_;
goto _start;
}
else
{
return v___y_1605_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0___boxed(lean_object* v_g_1613_, lean_object* v_x_1614_){
_start:
{
uint8_t v_res_1615_; lean_object* v_r_1616_; 
v_res_1615_ = lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0(v_g_1613_, v_x_1614_);
lean_dec(v_x_1614_);
lean_dec_ref(v_g_1613_);
v_r_1616_ = lean_box(v_res_1615_);
return v_r_1616_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_implied(lean_object* v_L_1617_, lean_object* v_g_1618_){
_start:
{
uint8_t v___x_1619_; 
v___x_1619_ = lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0(v_g_1618_, v_L_1617_);
return v___x_1619_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_implied___boxed(lean_object* v_L_1620_, lean_object* v_g_1621_){
_start:
{
uint8_t v_res_1622_; lean_object* v_r_1623_; 
v_res_1622_ = lp_clauseflow_ClauseFlow_implied(v_L_1620_, v_g_1621_);
lean_dec_ref(v_g_1621_);
lean_dec(v_L_1620_);
v_r_1623_ = lean_box(v_res_1622_);
return v_r_1623_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_fired_spec__0(lean_object* v_L_1624_, lean_object* v_x_1625_){
_start:
{
if (lean_obj_tag(v_x_1625_) == 0)
{
uint8_t v___x_1626_; 
v___x_1626_ = 1;
return v___x_1626_;
}
else
{
lean_object* v_head_1627_; lean_object* v_tail_1628_; uint8_t v___x_1629_; 
v_head_1627_ = lean_ctor_get(v_x_1625_, 0);
v_tail_1628_ = lean_ctor_get(v_x_1625_, 1);
v___x_1629_ = lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0(v_head_1627_, v_L_1624_);
if (v___x_1629_ == 0)
{
return v___x_1629_;
}
else
{
v_x_1625_ = v_tail_1628_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_fired_spec__0___boxed(lean_object* v_L_1631_, lean_object* v_x_1632_){
_start:
{
uint8_t v_res_1633_; lean_object* v_r_1634_; 
v_res_1633_ = lp_clauseflow_List_all___at___00ClauseFlow_fired_spec__0(v_L_1631_, v_x_1632_);
lean_dec(v_x_1632_);
lean_dec(v_L_1631_);
v_r_1634_ = lean_box(v_res_1633_);
return v_r_1634_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_fired_spec__1(lean_object* v_L_1635_, lean_object* v_a_1636_, lean_object* v_a_1637_){
_start:
{
if (lean_obj_tag(v_a_1636_) == 0)
{
lean_object* v___x_1638_; 
v___x_1638_ = l_List_reverse___redArg(v_a_1637_);
return v___x_1638_;
}
else
{
lean_object* v_head_1639_; lean_object* v_tail_1640_; lean_object* v___x_1642_; uint8_t v_isShared_1643_; uint8_t v_isSharedCheck_1651_; 
v_head_1639_ = lean_ctor_get(v_a_1636_, 0);
v_tail_1640_ = lean_ctor_get(v_a_1636_, 1);
v_isSharedCheck_1651_ = !lean_is_exclusive(v_a_1636_);
if (v_isSharedCheck_1651_ == 0)
{
v___x_1642_ = v_a_1636_;
v_isShared_1643_ = v_isSharedCheck_1651_;
goto v_resetjp_1641_;
}
else
{
lean_inc(v_tail_1640_);
lean_inc(v_head_1639_);
lean_dec(v_a_1636_);
v___x_1642_ = lean_box(0);
v_isShared_1643_ = v_isSharedCheck_1651_;
goto v_resetjp_1641_;
}
v_resetjp_1641_:
{
lean_object* v_gs_1644_; uint8_t v___x_1645_; 
v_gs_1644_ = lean_ctor_get(v_head_1639_, 0);
v___x_1645_ = lp_clauseflow_List_all___at___00ClauseFlow_fired_spec__0(v_L_1635_, v_gs_1644_);
if (v___x_1645_ == 0)
{
lean_del_object(v___x_1642_);
lean_dec(v_head_1639_);
v_a_1636_ = v_tail_1640_;
goto _start;
}
else
{
lean_object* v___x_1648_; 
if (v_isShared_1643_ == 0)
{
lean_ctor_set(v___x_1642_, 1, v_a_1637_);
v___x_1648_ = v___x_1642_;
goto v_reusejp_1647_;
}
else
{
lean_object* v_reuseFailAlloc_1650_; 
v_reuseFailAlloc_1650_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1650_, 0, v_head_1639_);
lean_ctor_set(v_reuseFailAlloc_1650_, 1, v_a_1637_);
v___x_1648_ = v_reuseFailAlloc_1650_;
goto v_reusejp_1647_;
}
v_reusejp_1647_:
{
v_a_1636_ = v_tail_1640_;
v_a_1637_ = v___x_1648_;
goto _start;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_fired_spec__1___boxed(lean_object* v_L_1652_, lean_object* v_a_1653_, lean_object* v_a_1654_){
_start:
{
lean_object* v_res_1655_; 
v_res_1655_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_fired_spec__1(v_L_1652_, v_a_1653_, v_a_1654_);
lean_dec(v_L_1652_);
return v_res_1655_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_fired(lean_object* v_a_1656_, lean_object* v_L_1657_){
_start:
{
lean_object* v_cls_1658_; lean_object* v___x_1659_; lean_object* v___x_1660_; 
v_cls_1658_ = lean_ctor_get(v_a_1656_, 1);
lean_inc(v_cls_1658_);
lean_dec_ref(v_a_1656_);
v___x_1659_ = lean_box(0);
v___x_1660_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_fired_spec__1(v_L_1657_, v_cls_1658_, v___x_1659_);
return v___x_1660_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_fired___boxed(lean_object* v_a_1661_, lean_object* v_L_1662_){
_start:
{
lean_object* v_res_1663_; 
v_res_1663_ = lp_clauseflow_ClauseFlow_fired(v_a_1661_, v_L_1662_);
lean_dec(v_L_1662_);
return v_res_1663_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_unitLits_spec__0(lean_object* v_L_1664_, uint8_t v___x_1665_, lean_object* v_a_1666_, lean_object* v_a_1667_){
_start:
{
if (lean_obj_tag(v_a_1666_) == 0)
{
lean_object* v___x_1668_; 
v___x_1668_ = l_List_reverse___redArg(v_a_1667_);
return v___x_1668_;
}
else
{
lean_object* v_head_1669_; lean_object* v_tail_1670_; lean_object* v___x_1672_; uint8_t v_isShared_1673_; uint8_t v_isSharedCheck_1681_; 
v_head_1669_ = lean_ctor_get(v_a_1666_, 0);
v_tail_1670_ = lean_ctor_get(v_a_1666_, 1);
v_isSharedCheck_1681_ = !lean_is_exclusive(v_a_1666_);
if (v_isSharedCheck_1681_ == 0)
{
v___x_1672_ = v_a_1666_;
v_isShared_1673_ = v_isSharedCheck_1681_;
goto v_resetjp_1671_;
}
else
{
lean_inc(v_tail_1670_);
lean_inc(v_head_1669_);
lean_dec(v_a_1666_);
v___x_1672_ = lean_box(0);
v_isShared_1673_ = v_isSharedCheck_1681_;
goto v_resetjp_1671_;
}
v_resetjp_1671_:
{
uint8_t v___x_1674_; 
v___x_1674_ = lp_clauseflow_List_any___at___00ClauseFlow_implied_spec__0(v_head_1669_, v_L_1664_);
if (v___x_1674_ == 0)
{
if (v___x_1665_ == 0)
{
lean_del_object(v___x_1672_);
lean_dec(v_head_1669_);
v_a_1666_ = v_tail_1670_;
goto _start;
}
else
{
lean_object* v___x_1677_; 
if (v_isShared_1673_ == 0)
{
lean_ctor_set(v___x_1672_, 1, v_a_1667_);
v___x_1677_ = v___x_1672_;
goto v_reusejp_1676_;
}
else
{
lean_object* v_reuseFailAlloc_1679_; 
v_reuseFailAlloc_1679_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1679_, 0, v_head_1669_);
lean_ctor_set(v_reuseFailAlloc_1679_, 1, v_a_1667_);
v___x_1677_ = v_reuseFailAlloc_1679_;
goto v_reusejp_1676_;
}
v_reusejp_1676_:
{
v_a_1666_ = v_tail_1670_;
v_a_1667_ = v___x_1677_;
goto _start;
}
}
}
else
{
lean_del_object(v___x_1672_);
lean_dec(v_head_1669_);
v_a_1666_ = v_tail_1670_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_unitLits_spec__0___boxed(lean_object* v_L_1682_, lean_object* v___x_1683_, lean_object* v_a_1684_, lean_object* v_a_1685_){
_start:
{
uint8_t v___x_196__boxed_1686_; lean_object* v_res_1687_; 
v___x_196__boxed_1686_ = lean_unbox(v___x_1683_);
v_res_1687_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_unitLits_spec__0(v_L_1682_, v___x_196__boxed_1686_, v_a_1684_, v_a_1685_);
lean_dec(v_L_1682_);
return v_res_1687_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_unitLits_spec__1(lean_object* v_L_1688_, lean_object* v_a_1689_, lean_object* v_a_1690_){
_start:
{
if (lean_obj_tag(v_a_1689_) == 0)
{
lean_object* v___x_1691_; 
v___x_1691_ = lean_array_to_list(v_a_1690_);
return v___x_1691_;
}
else
{
lean_object* v_head_1692_; lean_object* v_tail_1693_; lean_object* v___y_1695_; lean_object* v_gs_1698_; lean_object* v_c_1699_; uint8_t v___x_1700_; 
v_head_1692_ = lean_ctor_get(v_a_1689_, 0);
lean_inc(v_head_1692_);
v_tail_1693_ = lean_ctor_get(v_a_1689_, 1);
lean_inc(v_tail_1693_);
lean_dec_ref_known(v_a_1689_, 2);
v_gs_1698_ = lean_ctor_get(v_head_1692_, 0);
lean_inc(v_gs_1698_);
v_c_1699_ = lean_ctor_get(v_head_1692_, 1);
lean_inc(v_c_1699_);
lean_dec(v_head_1692_);
v___x_1700_ = lp_clauseflow_ClauseFlow_Concl_isBot(v_c_1699_);
lean_dec(v_c_1699_);
if (v___x_1700_ == 0)
{
lean_object* v___x_1701_; 
lean_dec(v_gs_1698_);
v___x_1701_ = lean_box(0);
v___y_1695_ = v___x_1701_;
goto v___jp_1694_;
}
else
{
lean_object* v___x_1702_; lean_object* v___x_1703_; 
v___x_1702_ = lean_box(0);
v___x_1703_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_unitLits_spec__0(v_L_1688_, v___x_1700_, v_gs_1698_, v___x_1702_);
if (lean_obj_tag(v___x_1703_) == 1)
{
lean_object* v_tail_1704_; 
v_tail_1704_ = lean_ctor_get(v___x_1703_, 1);
lean_inc(v_tail_1704_);
if (lean_obj_tag(v_tail_1704_) == 0)
{
lean_object* v_head_1705_; lean_object* v___x_1707_; uint8_t v_isShared_1708_; uint8_t v_isSharedCheck_1722_; 
v_head_1705_ = lean_ctor_get(v___x_1703_, 0);
v_isSharedCheck_1722_ = !lean_is_exclusive(v___x_1703_);
if (v_isSharedCheck_1722_ == 0)
{
lean_object* v_unused_1723_; 
v_unused_1723_ = lean_ctor_get(v___x_1703_, 1);
lean_dec(v_unused_1723_);
v___x_1707_ = v___x_1703_;
v_isShared_1708_ = v_isSharedCheck_1722_;
goto v_resetjp_1706_;
}
else
{
lean_inc(v_head_1705_);
lean_dec(v___x_1703_);
v___x_1707_ = lean_box(0);
v_isShared_1708_ = v_isSharedCheck_1722_;
goto v_resetjp_1706_;
}
v_resetjp_1706_:
{
lean_object* v_fst_1709_; lean_object* v_snd_1710_; lean_object* v___x_1712_; uint8_t v_isShared_1713_; uint8_t v_isSharedCheck_1721_; 
v_fst_1709_ = lean_ctor_get(v_head_1705_, 0);
v_snd_1710_ = lean_ctor_get(v_head_1705_, 1);
v_isSharedCheck_1721_ = !lean_is_exclusive(v_head_1705_);
if (v_isSharedCheck_1721_ == 0)
{
v___x_1712_ = v_head_1705_;
v_isShared_1713_ = v_isSharedCheck_1721_;
goto v_resetjp_1711_;
}
else
{
lean_inc(v_snd_1710_);
lean_inc(v_fst_1709_);
lean_dec(v_head_1705_);
v___x_1712_ = lean_box(0);
v_isShared_1713_ = v_isSharedCheck_1721_;
goto v_resetjp_1711_;
}
v_resetjp_1711_:
{
lean_object* v___x_1714_; lean_object* v___x_1716_; 
v___x_1714_ = lp_clauseflow_ClauseFlow_Test_neg(v_snd_1710_);
if (v_isShared_1713_ == 0)
{
lean_ctor_set(v___x_1712_, 1, v___x_1714_);
v___x_1716_ = v___x_1712_;
goto v_reusejp_1715_;
}
else
{
lean_object* v_reuseFailAlloc_1720_; 
v_reuseFailAlloc_1720_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1720_, 0, v_fst_1709_);
lean_ctor_set(v_reuseFailAlloc_1720_, 1, v___x_1714_);
v___x_1716_ = v_reuseFailAlloc_1720_;
goto v_reusejp_1715_;
}
v_reusejp_1715_:
{
lean_object* v___x_1718_; 
if (v_isShared_1708_ == 0)
{
lean_ctor_set(v___x_1707_, 0, v___x_1716_);
v___x_1718_ = v___x_1707_;
goto v_reusejp_1717_;
}
else
{
lean_object* v_reuseFailAlloc_1719_; 
v_reuseFailAlloc_1719_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1719_, 0, v___x_1716_);
lean_ctor_set(v_reuseFailAlloc_1719_, 1, v_tail_1704_);
v___x_1718_ = v_reuseFailAlloc_1719_;
goto v_reusejp_1717_;
}
v_reusejp_1717_:
{
v___y_1695_ = v___x_1718_;
goto v___jp_1694_;
}
}
}
}
}
else
{
lean_dec_ref_known(v___x_1703_, 2);
lean_dec(v_tail_1704_);
v___y_1695_ = v___x_1702_;
goto v___jp_1694_;
}
}
else
{
lean_dec(v___x_1703_);
v___y_1695_ = v___x_1702_;
goto v___jp_1694_;
}
}
v___jp_1694_:
{
lean_object* v___x_1696_; 
v___x_1696_ = l_List_foldl___at___00Array_appendList_spec__0___redArg(v_a_1690_, v___y_1695_);
v_a_1689_ = v_tail_1693_;
v_a_1690_ = v___x_1696_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_unitLits_spec__1___boxed(lean_object* v_L_1724_, lean_object* v_a_1725_, lean_object* v_a_1726_){
_start:
{
lean_object* v_res_1727_; 
v_res_1727_ = lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_unitLits_spec__1(v_L_1724_, v_a_1725_, v_a_1726_);
lean_dec(v_L_1724_);
return v_res_1727_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_unitLits(lean_object* v_a_1730_, lean_object* v_L_1731_){
_start:
{
lean_object* v_cls_1732_; lean_object* v___x_1733_; lean_object* v___x_1734_; 
v_cls_1732_ = lean_ctor_get(v_a_1730_, 1);
lean_inc(v_cls_1732_);
lean_dec_ref(v_a_1730_);
v___x_1733_ = ((lean_object*)(lp_clauseflow_ClauseFlow_unitLits___closed__0));
v___x_1734_ = lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_unitLits_spec__1(v_L_1731_, v_cls_1732_, v___x_1733_);
return v___x_1734_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_unitLits___boxed(lean_object* v_a_1735_, lean_object* v_L_1736_){
_start:
{
lean_object* v_res_1737_; 
v_res_1737_ = lp_clauseflow_ClauseFlow_unitLits(v_a_1735_, v_L_1736_);
lean_dec(v_L_1736_);
return v_res_1737_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_closure(lean_object* v_a_1738_, lean_object* v_L_1739_){
_start:
{
lean_object* v___x_1740_; lean_object* v___x_1741_; lean_object* v___x_1742_; lean_object* v___x_1743_; 
lean_inc_ref(v_a_1738_);
v___x_1740_ = lp_clauseflow_ClauseFlow_unitLits(v_a_1738_, v_L_1739_);
v___x_1741_ = l_List_appendTR___redArg(v___x_1740_, v_L_1739_);
v___x_1742_ = lp_clauseflow_ClauseFlow_unitLits(v_a_1738_, v___x_1741_);
v___x_1743_ = l_List_appendTR___redArg(v___x_1742_, v___x_1741_);
return v___x_1743_;
}
}
static lean_object* _init_lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0(void){
_start:
{
lean_object* v___x_1744_; lean_object* v___f_1745_; 
v___x_1744_ = lean_alloc_closure((void*)(l_instDecidableEqNat___boxed), 2, 0);
v___f_1745_ = lean_alloc_closure((void*)(l_instBEqOfDecidableEq___redArg___lam__0___boxed), 3, 1);
lean_closure_set(v___f_1745_, 0, v___x_1744_);
return v___f_1745_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1(lean_object* v_k_1746_, lean_object* v_x_1747_){
_start:
{
if (lean_obj_tag(v_x_1747_) == 0)
{
uint8_t v___x_1748_; 
lean_dec(v_k_1746_);
v___x_1748_ = 0;
return v___x_1748_;
}
else
{
lean_object* v_head_1749_; lean_object* v_tail_1750_; lean_object* v_c_1751_; lean_object* v___f_1752_; lean_object* v___x_1753_; uint8_t v___x_1754_; 
v_head_1749_ = lean_ctor_get(v_x_1747_, 0);
v_tail_1750_ = lean_ctor_get(v_x_1747_, 1);
v_c_1751_ = lean_ctor_get(v_head_1749_, 1);
v___f_1752_ = lean_obj_once(&lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0, &lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0_once, _init_lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0);
v___x_1753_ = lp_clauseflow_ClauseFlow_Concl_hs(v_c_1751_);
lean_inc(v_k_1746_);
v___x_1754_ = l_List_elem___redArg(v___f_1752_, v_k_1746_, v___x_1753_);
if (v___x_1754_ == 0)
{
v_x_1747_ = v_tail_1750_;
goto _start;
}
else
{
lean_dec(v_k_1746_);
return v___x_1754_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___boxed(lean_object* v_k_1756_, lean_object* v_x_1757_){
_start:
{
uint8_t v_res_1758_; lean_object* v_r_1759_; 
v_res_1758_ = lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1(v_k_1756_, v_x_1757_);
lean_dec(v_x_1757_);
v_r_1759_ = lean_box(v_res_1758_);
return v_r_1759_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__2(lean_object* v_a_1760_, lean_object* v_L_1761_, lean_object* v_x_1762_){
_start:
{
if (lean_obj_tag(v_x_1762_) == 0)
{
uint8_t v___x_1763_; 
lean_dec_ref(v_a_1760_);
v___x_1763_ = 1;
return v___x_1763_;
}
else
{
lean_object* v_head_1764_; lean_object* v_tail_1765_; lean_object* v___x_1766_; uint8_t v___x_1767_; 
v_head_1764_ = lean_ctor_get(v_x_1762_, 0);
lean_inc(v_head_1764_);
v_tail_1765_ = lean_ctor_get(v_x_1762_, 1);
lean_inc(v_tail_1765_);
lean_dec_ref_known(v_x_1762_, 2);
lean_inc_ref(v_a_1760_);
v___x_1766_ = lp_clauseflow_ClauseFlow_fired(v_a_1760_, v_L_1761_);
v___x_1767_ = lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1(v_head_1764_, v___x_1766_);
lean_dec(v___x_1766_);
if (v___x_1767_ == 0)
{
lean_dec(v_tail_1765_);
lean_dec_ref(v_a_1760_);
return v___x_1767_;
}
else
{
v_x_1762_ = v_tail_1765_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__2___boxed(lean_object* v_a_1769_, lean_object* v_L_1770_, lean_object* v_x_1771_){
_start:
{
uint8_t v_res_1772_; lean_object* v_r_1773_; 
v_res_1772_ = lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__2(v_a_1769_, v_L_1770_, v_x_1771_);
lean_dec(v_L_1770_);
v_r_1773_ = lean_box(v_res_1772_);
return v_r_1773_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__4(lean_object* v_x_1774_){
_start:
{
if (lean_obj_tag(v_x_1774_) == 0)
{
uint8_t v___x_1775_; 
v___x_1775_ = 0;
return v___x_1775_;
}
else
{
lean_object* v_head_1776_; lean_object* v_tail_1777_; lean_object* v_c_1778_; uint8_t v___x_1779_; 
v_head_1776_ = lean_ctor_get(v_x_1774_, 0);
v_tail_1777_ = lean_ctor_get(v_x_1774_, 1);
v_c_1778_ = lean_ctor_get(v_head_1776_, 1);
v___x_1779_ = lp_clauseflow_ClauseFlow_Concl_isBot(v_c_1778_);
if (v___x_1779_ == 0)
{
v_x_1774_ = v_tail_1777_;
goto _start;
}
else
{
return v___x_1779_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__4___boxed(lean_object* v_x_1781_){
_start:
{
uint8_t v_res_1782_; lean_object* v_r_1783_; 
v_res_1782_ = lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__4(v_x_1781_);
lean_dec(v_x_1781_);
v_r_1783_ = lean_box(v_res_1782_);
return v_r_1783_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__0(lean_object* v_k_1784_, lean_object* v_x_1785_){
_start:
{
if (lean_obj_tag(v_x_1785_) == 0)
{
uint8_t v___x_1786_; 
lean_dec(v_k_1784_);
v___x_1786_ = 0;
return v___x_1786_;
}
else
{
lean_object* v_head_1787_; lean_object* v_tail_1788_; uint8_t v___y_1790_; lean_object* v_c_1792_; lean_object* v___f_1793_; lean_object* v___x_1794_; uint8_t v___x_1795_; 
v_head_1787_ = lean_ctor_get(v_x_1785_, 0);
v_tail_1788_ = lean_ctor_get(v_x_1785_, 1);
v_c_1792_ = lean_ctor_get(v_head_1787_, 1);
v___f_1793_ = lean_obj_once(&lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0, &lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0_once, _init_lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0);
v___x_1794_ = lp_clauseflow_ClauseFlow_Concl_ps(v_c_1792_);
lean_inc(v_k_1784_);
v___x_1795_ = l_List_elem___redArg(v___f_1793_, v_k_1784_, v___x_1794_);
if (v___x_1795_ == 0)
{
lean_object* v___x_1796_; uint8_t v___x_1797_; 
v___x_1796_ = lp_clauseflow_ClauseFlow_Concl_hs(v_c_1792_);
lean_inc(v_k_1784_);
v___x_1797_ = l_List_elem___redArg(v___f_1793_, v_k_1784_, v___x_1796_);
v___y_1790_ = v___x_1797_;
goto v___jp_1789_;
}
else
{
v___y_1790_ = v___x_1795_;
goto v___jp_1789_;
}
v___jp_1789_:
{
if (v___y_1790_ == 0)
{
v_x_1785_ = v_tail_1788_;
goto _start;
}
else
{
lean_dec(v_k_1784_);
return v___y_1790_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__0___boxed(lean_object* v_k_1798_, lean_object* v_x_1799_){
_start:
{
uint8_t v_res_1800_; lean_object* v_r_1801_; 
v_res_1800_ = lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__0(v_k_1798_, v_x_1799_);
lean_dec(v_x_1799_);
v_r_1801_ = lean_box(v_res_1800_);
return v_r_1801_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__3(lean_object* v_a_1802_, lean_object* v_L_1803_, lean_object* v_x_1804_){
_start:
{
if (lean_obj_tag(v_x_1804_) == 0)
{
uint8_t v___x_1805_; 
lean_dec_ref(v_a_1802_);
v___x_1805_ = 1;
return v___x_1805_;
}
else
{
lean_object* v_head_1806_; lean_object* v_tail_1807_; lean_object* v___x_1808_; uint8_t v___x_1809_; 
v_head_1806_ = lean_ctor_get(v_x_1804_, 0);
lean_inc(v_head_1806_);
v_tail_1807_ = lean_ctor_get(v_x_1804_, 1);
lean_inc(v_tail_1807_);
lean_dec_ref_known(v_x_1804_, 2);
lean_inc_ref(v_a_1802_);
v___x_1808_ = lp_clauseflow_ClauseFlow_fired(v_a_1802_, v_L_1803_);
v___x_1809_ = lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__0(v_head_1806_, v___x_1808_);
lean_dec(v___x_1808_);
if (v___x_1809_ == 0)
{
lean_dec(v_tail_1807_);
lean_dec_ref(v_a_1802_);
return v___x_1809_;
}
else
{
v_x_1804_ = v_tail_1807_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__3___boxed(lean_object* v_a_1811_, lean_object* v_L_1812_, lean_object* v_x_1813_){
_start:
{
uint8_t v_res_1814_; lean_object* v_r_1815_; 
v_res_1814_ = lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__3(v_a_1811_, v_L_1812_, v_x_1813_);
lean_dec(v_L_1812_);
v_r_1815_ = lean_box(v_res_1814_);
return v_r_1815_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_entailsCl(lean_object* v_a_1816_, lean_object* v_c_1817_){
_start:
{
lean_object* v_gs_1818_; lean_object* v_c_1819_; lean_object* v_facts_1820_; lean_object* v___x_1821_; lean_object* v_L_1822_; uint8_t v___y_1824_; uint8_t v___x_1829_; 
v_gs_1818_ = lean_ctor_get(v_c_1817_, 0);
lean_inc(v_gs_1818_);
v_c_1819_ = lean_ctor_get(v_c_1817_, 1);
lean_inc(v_c_1819_);
lean_dec_ref(v_c_1817_);
v_facts_1820_ = lean_ctor_get(v_a_1816_, 0);
lean_inc(v_facts_1820_);
v___x_1821_ = l_List_appendTR___redArg(v_gs_1818_, v_facts_1820_);
lean_inc_ref(v_a_1816_);
v_L_1822_ = lp_clauseflow_ClauseFlow_closure(v_a_1816_, v___x_1821_);
v___x_1829_ = lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1(v_L_1822_, v_L_1822_);
if (v___x_1829_ == 0)
{
lean_object* v___x_1830_; uint8_t v___x_1831_; 
lean_inc_ref(v_a_1816_);
v___x_1830_ = lp_clauseflow_ClauseFlow_fired(v_a_1816_, v_L_1822_);
v___x_1831_ = lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__4(v___x_1830_);
lean_dec(v___x_1830_);
v___y_1824_ = v___x_1831_;
goto v___jp_1823_;
}
else
{
v___y_1824_ = v___x_1829_;
goto v___jp_1823_;
}
v___jp_1823_:
{
if (v___y_1824_ == 0)
{
if (lean_obj_tag(v_c_1819_) == 0)
{
lean_dec(v_L_1822_);
lean_dec_ref(v_a_1816_);
return v___y_1824_;
}
else
{
lean_object* v_hs_1825_; lean_object* v_ps_1826_; uint8_t v___x_1827_; 
v_hs_1825_ = lean_ctor_get(v_c_1819_, 0);
lean_inc(v_hs_1825_);
v_ps_1826_ = lean_ctor_get(v_c_1819_, 1);
lean_inc(v_ps_1826_);
lean_dec_ref_known(v_c_1819_, 2);
lean_inc_ref(v_a_1816_);
v___x_1827_ = lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__2(v_a_1816_, v_L_1822_, v_hs_1825_);
if (v___x_1827_ == 0)
{
lean_dec(v_ps_1826_);
lean_dec(v_L_1822_);
lean_dec_ref(v_a_1816_);
return v___x_1827_;
}
else
{
uint8_t v___x_1828_; 
v___x_1828_ = lp_clauseflow_List_all___at___00ClauseFlow_entailsCl_spec__3(v_a_1816_, v_L_1822_, v_ps_1826_);
lean_dec(v_L_1822_);
return v___x_1828_;
}
}
}
else
{
lean_dec(v_L_1822_);
lean_dec(v_c_1819_);
lean_dec_ref(v_a_1816_);
return v___y_1824_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_entailsCl___boxed(lean_object* v_a_1832_, lean_object* v_c_1833_){
_start:
{
uint8_t v_res_1834_; lean_object* v_r_1835_; 
v_res_1834_ = lp_clauseflow_ClauseFlow_entailsCl(v_a_1832_, v_c_1833_);
v_r_1835_ = lean_box(v_res_1834_);
return v_r_1835_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_factCl(lean_object* v_f_1836_){
_start:
{
lean_object* v_fst_1837_; lean_object* v_snd_1838_; lean_object* v___x_1840_; uint8_t v_isShared_1841_; uint8_t v_isSharedCheck_1850_; 
v_fst_1837_ = lean_ctor_get(v_f_1836_, 0);
v_snd_1838_ = lean_ctor_get(v_f_1836_, 1);
v_isSharedCheck_1850_ = !lean_is_exclusive(v_f_1836_);
if (v_isSharedCheck_1850_ == 0)
{
v___x_1840_ = v_f_1836_;
v_isShared_1841_ = v_isSharedCheck_1850_;
goto v_resetjp_1839_;
}
else
{
lean_inc(v_snd_1838_);
lean_inc(v_fst_1837_);
lean_dec(v_f_1836_);
v___x_1840_ = lean_box(0);
v_isShared_1841_ = v_isSharedCheck_1850_;
goto v_resetjp_1839_;
}
v_resetjp_1839_:
{
lean_object* v___x_1842_; lean_object* v___x_1844_; 
v___x_1842_ = lp_clauseflow_ClauseFlow_Test_neg(v_snd_1838_);
if (v_isShared_1841_ == 0)
{
lean_ctor_set(v___x_1840_, 1, v___x_1842_);
v___x_1844_ = v___x_1840_;
goto v_reusejp_1843_;
}
else
{
lean_object* v_reuseFailAlloc_1849_; 
v_reuseFailAlloc_1849_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1849_, 0, v_fst_1837_);
lean_ctor_set(v_reuseFailAlloc_1849_, 1, v___x_1842_);
v___x_1844_ = v_reuseFailAlloc_1849_;
goto v_reusejp_1843_;
}
v_reusejp_1843_:
{
lean_object* v___x_1845_; lean_object* v___x_1846_; lean_object* v___x_1847_; lean_object* v___x_1848_; 
v___x_1845_ = lean_box(0);
v___x_1846_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_1846_, 0, v___x_1844_);
lean_ctor_set(v___x_1846_, 1, v___x_1845_);
v___x_1847_ = lean_box(0);
v___x_1848_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_1848_, 0, v___x_1846_);
lean_ctor_set(v___x_1848_, 1, v___x_1847_);
return v___x_1848_;
}
}
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_le_spec__0(lean_object* v_a_1851_, lean_object* v_x_1852_){
_start:
{
if (lean_obj_tag(v_x_1852_) == 0)
{
uint8_t v___x_1853_; 
lean_dec_ref(v_a_1851_);
v___x_1853_ = 1;
return v___x_1853_;
}
else
{
lean_object* v_head_1854_; lean_object* v_tail_1855_; lean_object* v___x_1856_; uint8_t v___x_1857_; 
v_head_1854_ = lean_ctor_get(v_x_1852_, 0);
lean_inc(v_head_1854_);
v_tail_1855_ = lean_ctor_get(v_x_1852_, 1);
lean_inc(v_tail_1855_);
lean_dec_ref_known(v_x_1852_, 2);
v___x_1856_ = lp_clauseflow_ClauseFlow_factCl(v_head_1854_);
lean_inc_ref(v_a_1851_);
v___x_1857_ = lp_clauseflow_ClauseFlow_entailsCl(v_a_1851_, v___x_1856_);
if (v___x_1857_ == 0)
{
lean_dec(v_tail_1855_);
lean_dec_ref(v_a_1851_);
return v___x_1857_;
}
else
{
v_x_1852_ = v_tail_1855_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_le_spec__0___boxed(lean_object* v_a_1859_, lean_object* v_x_1860_){
_start:
{
uint8_t v_res_1861_; lean_object* v_r_1862_; 
v_res_1861_ = lp_clauseflow_List_all___at___00ClauseFlow_le_spec__0(v_a_1859_, v_x_1860_);
v_r_1862_ = lean_box(v_res_1861_);
return v_r_1862_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1(lean_object* v_a_1863_, lean_object* v_x_1864_){
_start:
{
if (lean_obj_tag(v_x_1864_) == 0)
{
uint8_t v___x_1865_; 
lean_dec_ref(v_a_1863_);
v___x_1865_ = 1;
return v___x_1865_;
}
else
{
lean_object* v_head_1866_; lean_object* v_tail_1867_; uint8_t v___x_1868_; 
v_head_1866_ = lean_ctor_get(v_x_1864_, 0);
lean_inc(v_head_1866_);
v_tail_1867_ = lean_ctor_get(v_x_1864_, 1);
lean_inc(v_tail_1867_);
lean_dec_ref_known(v_x_1864_, 2);
lean_inc_ref(v_a_1863_);
v___x_1868_ = lp_clauseflow_ClauseFlow_entailsCl(v_a_1863_, v_head_1866_);
if (v___x_1868_ == 0)
{
lean_dec(v_tail_1867_);
lean_dec_ref(v_a_1863_);
return v___x_1868_;
}
else
{
v_x_1864_ = v_tail_1867_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1___boxed(lean_object* v_a_1870_, lean_object* v_x_1871_){
_start:
{
uint8_t v_res_1872_; lean_object* v_r_1873_; 
v_res_1872_ = lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1(v_a_1870_, v_x_1871_);
v_r_1873_ = lean_box(v_res_1872_);
return v_r_1873_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_le(lean_object* v_a_1874_, lean_object* v_b_1875_){
_start:
{
lean_object* v_facts_1876_; lean_object* v_cls_1877_; uint8_t v___x_1878_; 
v_facts_1876_ = lean_ctor_get(v_b_1875_, 0);
lean_inc(v_facts_1876_);
v_cls_1877_ = lean_ctor_get(v_b_1875_, 1);
lean_inc(v_cls_1877_);
lean_dec_ref(v_b_1875_);
lean_inc_ref(v_a_1874_);
v___x_1878_ = lp_clauseflow_List_all___at___00ClauseFlow_le_spec__0(v_a_1874_, v_facts_1876_);
if (v___x_1878_ == 0)
{
lean_dec(v_cls_1877_);
lean_dec_ref(v_a_1874_);
return v___x_1878_;
}
else
{
uint8_t v___x_1879_; 
v___x_1879_ = lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1(v_a_1874_, v_cls_1877_);
return v___x_1879_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_le___boxed(lean_object* v_a_1880_, lean_object* v_b_1881_){
_start:
{
uint8_t v_res_1882_; lean_object* v_r_1883_; 
v_res_1882_ = lp_clauseflow_ClauseFlow_le(v_a_1880_, v_b_1881_);
v_r_1883_ = lean_box(v_res_1882_);
return v_r_1883_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_any___at___00ClauseFlow_mentions_spec__0(lean_object* v_l_1884_, lean_object* v_x_1885_){
_start:
{
if (lean_obj_tag(v_x_1885_) == 0)
{
uint8_t v___x_1886_; 
v___x_1886_ = 0;
return v___x_1886_;
}
else
{
lean_object* v_head_1887_; lean_object* v_tail_1888_; lean_object* v_fst_1889_; uint8_t v___x_1890_; 
v_head_1887_ = lean_ctor_get(v_x_1885_, 0);
v_tail_1888_ = lean_ctor_get(v_x_1885_, 1);
v_fst_1889_ = lean_ctor_get(v_head_1887_, 0);
v___x_1890_ = lean_nat_dec_eq(v_fst_1889_, v_l_1884_);
if (v___x_1890_ == 0)
{
v_x_1885_ = v_tail_1888_;
goto _start;
}
else
{
return v___x_1890_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_any___at___00ClauseFlow_mentions_spec__0___boxed(lean_object* v_l_1892_, lean_object* v_x_1893_){
_start:
{
uint8_t v_res_1894_; lean_object* v_r_1895_; 
v_res_1894_ = lp_clauseflow_List_any___at___00ClauseFlow_mentions_spec__0(v_l_1892_, v_x_1893_);
lean_dec(v_x_1893_);
lean_dec(v_l_1892_);
v_r_1895_ = lean_box(v_res_1894_);
return v_r_1895_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_mentions(lean_object* v_l_1896_, lean_object* v_c_1897_){
_start:
{
lean_object* v_gs_1898_; uint8_t v___x_1899_; 
v_gs_1898_ = lean_ctor_get(v_c_1897_, 0);
v___x_1899_ = lp_clauseflow_List_any___at___00ClauseFlow_mentions_spec__0(v_l_1896_, v_gs_1898_);
return v___x_1899_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_mentions___boxed(lean_object* v_l_1900_, lean_object* v_c_1901_){
_start:
{
uint8_t v_res_1902_; lean_object* v_r_1903_; 
v_res_1902_ = lp_clauseflow_ClauseFlow_mentions(v_l_1900_, v_c_1901_);
lean_dec_ref(v_c_1901_);
lean_dec(v_l_1900_);
v_r_1903_ = lean_box(v_res_1902_);
return v_r_1903_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__1(lean_object* v_l_1904_, lean_object* v_a_1905_, lean_object* v_a_1906_){
_start:
{
if (lean_obj_tag(v_a_1905_) == 0)
{
lean_object* v___x_1907_; 
v___x_1907_ = l_List_reverse___redArg(v_a_1906_);
return v___x_1907_;
}
else
{
lean_object* v_head_1908_; lean_object* v_tail_1909_; lean_object* v___x_1911_; uint8_t v_isShared_1912_; uint8_t v_isSharedCheck_1919_; 
v_head_1908_ = lean_ctor_get(v_a_1905_, 0);
v_tail_1909_ = lean_ctor_get(v_a_1905_, 1);
v_isSharedCheck_1919_ = !lean_is_exclusive(v_a_1905_);
if (v_isSharedCheck_1919_ == 0)
{
v___x_1911_ = v_a_1905_;
v_isShared_1912_ = v_isSharedCheck_1919_;
goto v_resetjp_1910_;
}
else
{
lean_inc(v_tail_1909_);
lean_inc(v_head_1908_);
lean_dec(v_a_1905_);
v___x_1911_ = lean_box(0);
v_isShared_1912_ = v_isSharedCheck_1919_;
goto v_resetjp_1910_;
}
v_resetjp_1910_:
{
uint8_t v___x_1913_; 
v___x_1913_ = lp_clauseflow_ClauseFlow_mentions(v_l_1904_, v_head_1908_);
if (v___x_1913_ == 0)
{
lean_object* v___x_1915_; 
if (v_isShared_1912_ == 0)
{
lean_ctor_set(v___x_1911_, 1, v_a_1906_);
v___x_1915_ = v___x_1911_;
goto v_reusejp_1914_;
}
else
{
lean_object* v_reuseFailAlloc_1917_; 
v_reuseFailAlloc_1917_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1917_, 0, v_head_1908_);
lean_ctor_set(v_reuseFailAlloc_1917_, 1, v_a_1906_);
v___x_1915_ = v_reuseFailAlloc_1917_;
goto v_reusejp_1914_;
}
v_reusejp_1914_:
{
v_a_1905_ = v_tail_1909_;
v_a_1906_ = v___x_1915_;
goto _start;
}
}
else
{
lean_del_object(v___x_1911_);
lean_dec(v_head_1908_);
v_a_1905_ = v_tail_1909_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__1___boxed(lean_object* v_l_1920_, lean_object* v_a_1921_, lean_object* v_a_1922_){
_start:
{
lean_object* v_res_1923_; 
v_res_1923_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__1(v_l_1920_, v_a_1921_, v_a_1922_);
lean_dec(v_l_1920_);
return v_res_1923_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__0(lean_object* v_l_1924_, lean_object* v_a_1925_, lean_object* v_a_1926_){
_start:
{
if (lean_obj_tag(v_a_1925_) == 0)
{
lean_object* v___x_1927_; 
v___x_1927_ = l_List_reverse___redArg(v_a_1926_);
return v___x_1927_;
}
else
{
lean_object* v_head_1928_; lean_object* v_tail_1929_; lean_object* v___x_1931_; uint8_t v_isShared_1932_; uint8_t v_isSharedCheck_1940_; 
v_head_1928_ = lean_ctor_get(v_a_1925_, 0);
v_tail_1929_ = lean_ctor_get(v_a_1925_, 1);
v_isSharedCheck_1940_ = !lean_is_exclusive(v_a_1925_);
if (v_isSharedCheck_1940_ == 0)
{
v___x_1931_ = v_a_1925_;
v_isShared_1932_ = v_isSharedCheck_1940_;
goto v_resetjp_1930_;
}
else
{
lean_inc(v_tail_1929_);
lean_inc(v_head_1928_);
lean_dec(v_a_1925_);
v___x_1931_ = lean_box(0);
v_isShared_1932_ = v_isSharedCheck_1940_;
goto v_resetjp_1930_;
}
v_resetjp_1930_:
{
lean_object* v_fst_1933_; uint8_t v___x_1934_; 
v_fst_1933_ = lean_ctor_get(v_head_1928_, 0);
v___x_1934_ = lean_nat_dec_eq(v_fst_1933_, v_l_1924_);
if (v___x_1934_ == 0)
{
lean_object* v___x_1936_; 
if (v_isShared_1932_ == 0)
{
lean_ctor_set(v___x_1931_, 1, v_a_1926_);
v___x_1936_ = v___x_1931_;
goto v_reusejp_1935_;
}
else
{
lean_object* v_reuseFailAlloc_1938_; 
v_reuseFailAlloc_1938_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1938_, 0, v_head_1928_);
lean_ctor_set(v_reuseFailAlloc_1938_, 1, v_a_1926_);
v___x_1936_ = v_reuseFailAlloc_1938_;
goto v_reusejp_1935_;
}
v_reusejp_1935_:
{
v_a_1925_ = v_tail_1929_;
v_a_1926_ = v___x_1936_;
goto _start;
}
}
else
{
lean_del_object(v___x_1931_);
lean_dec(v_head_1928_);
v_a_1925_ = v_tail_1929_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__0___boxed(lean_object* v_l_1941_, lean_object* v_a_1942_, lean_object* v_a_1943_){
_start:
{
lean_object* v_res_1944_; 
v_res_1944_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__0(v_l_1941_, v_a_1942_, v_a_1943_);
lean_dec(v_l_1941_);
return v_res_1944_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_kill(lean_object* v_l_1945_, lean_object* v_a_1946_){
_start:
{
lean_object* v_facts_1947_; lean_object* v_cls_1948_; lean_object* v___x_1950_; uint8_t v_isShared_1951_; uint8_t v_isSharedCheck_1958_; 
v_facts_1947_ = lean_ctor_get(v_a_1946_, 0);
v_cls_1948_ = lean_ctor_get(v_a_1946_, 1);
v_isSharedCheck_1958_ = !lean_is_exclusive(v_a_1946_);
if (v_isSharedCheck_1958_ == 0)
{
v___x_1950_ = v_a_1946_;
v_isShared_1951_ = v_isSharedCheck_1958_;
goto v_resetjp_1949_;
}
else
{
lean_inc(v_cls_1948_);
lean_inc(v_facts_1947_);
lean_dec(v_a_1946_);
v___x_1950_ = lean_box(0);
v_isShared_1951_ = v_isSharedCheck_1958_;
goto v_resetjp_1949_;
}
v_resetjp_1949_:
{
lean_object* v___x_1952_; lean_object* v___x_1953_; lean_object* v___x_1954_; lean_object* v___x_1956_; 
v___x_1952_ = lean_box(0);
v___x_1953_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__0(v_l_1945_, v_facts_1947_, v___x_1952_);
v___x_1954_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_kill_spec__1(v_l_1945_, v_cls_1948_, v___x_1952_);
if (v_isShared_1951_ == 0)
{
lean_ctor_set(v___x_1950_, 1, v___x_1954_);
lean_ctor_set(v___x_1950_, 0, v___x_1953_);
v___x_1956_ = v___x_1950_;
goto v_reusejp_1955_;
}
else
{
lean_object* v_reuseFailAlloc_1957_; 
v_reuseFailAlloc_1957_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1957_, 0, v___x_1953_);
lean_ctor_set(v_reuseFailAlloc_1957_, 1, v___x_1954_);
v___x_1956_ = v_reuseFailAlloc_1957_;
goto v_reusejp_1955_;
}
v_reusejp_1955_:
{
return v___x_1956_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_kill___boxed(lean_object* v_l_1959_, lean_object* v_a_1960_){
_start:
{
lean_object* v_res_1961_; 
v_res_1961_ = lp_clauseflow_ClauseFlow_kill(v_l_1959_, v_a_1960_);
lean_dec(v_l_1959_);
return v_res_1961_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_Concl_restrict_spec__0(lean_object* v_same_1962_, lean_object* v_a_1963_, lean_object* v_a_1964_){
_start:
{
if (lean_obj_tag(v_a_1963_) == 0)
{
lean_object* v___x_1965_; 
lean_dec(v_same_1962_);
v___x_1965_ = l_List_reverse___redArg(v_a_1964_);
return v___x_1965_;
}
else
{
lean_object* v_head_1966_; lean_object* v_tail_1967_; lean_object* v___x_1969_; uint8_t v_isShared_1970_; uint8_t v_isSharedCheck_1978_; 
v_head_1966_ = lean_ctor_get(v_a_1963_, 0);
v_tail_1967_ = lean_ctor_get(v_a_1963_, 1);
v_isSharedCheck_1978_ = !lean_is_exclusive(v_a_1963_);
if (v_isSharedCheck_1978_ == 0)
{
v___x_1969_ = v_a_1963_;
v_isShared_1970_ = v_isSharedCheck_1978_;
goto v_resetjp_1968_;
}
else
{
lean_inc(v_tail_1967_);
lean_inc(v_head_1966_);
lean_dec(v_a_1963_);
v___x_1969_ = lean_box(0);
v_isShared_1970_ = v_isSharedCheck_1978_;
goto v_resetjp_1968_;
}
v_resetjp_1968_:
{
lean_object* v___f_1971_; uint8_t v___x_1972_; 
v___f_1971_ = lean_obj_once(&lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0, &lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0_once, _init_lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0);
lean_inc(v_same_1962_);
lean_inc(v_head_1966_);
v___x_1972_ = l_List_elem___redArg(v___f_1971_, v_head_1966_, v_same_1962_);
if (v___x_1972_ == 0)
{
lean_del_object(v___x_1969_);
lean_dec(v_head_1966_);
v_a_1963_ = v_tail_1967_;
goto _start;
}
else
{
lean_object* v___x_1975_; 
if (v_isShared_1970_ == 0)
{
lean_ctor_set(v___x_1969_, 1, v_a_1964_);
v___x_1975_ = v___x_1969_;
goto v_reusejp_1974_;
}
else
{
lean_object* v_reuseFailAlloc_1977_; 
v_reuseFailAlloc_1977_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1977_, 0, v_head_1966_);
lean_ctor_set(v_reuseFailAlloc_1977_, 1, v_a_1964_);
v___x_1975_ = v_reuseFailAlloc_1977_;
goto v_reusejp_1974_;
}
v_reusejp_1974_:
{
v_a_1963_ = v_tail_1967_;
v_a_1964_ = v___x_1975_;
goto _start;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_Concl_restrict(lean_object* v_same_1979_, lean_object* v_x_1980_){
_start:
{
if (lean_obj_tag(v_x_1980_) == 0)
{
lean_dec(v_same_1979_);
return v_x_1980_;
}
else
{
lean_object* v_hs_1981_; lean_object* v_ps_1982_; lean_object* v___x_1984_; uint8_t v_isShared_1985_; uint8_t v_isSharedCheck_1992_; 
v_hs_1981_ = lean_ctor_get(v_x_1980_, 0);
v_ps_1982_ = lean_ctor_get(v_x_1980_, 1);
v_isSharedCheck_1992_ = !lean_is_exclusive(v_x_1980_);
if (v_isSharedCheck_1992_ == 0)
{
v___x_1984_ = v_x_1980_;
v_isShared_1985_ = v_isSharedCheck_1992_;
goto v_resetjp_1983_;
}
else
{
lean_inc(v_ps_1982_);
lean_inc(v_hs_1981_);
lean_dec(v_x_1980_);
v___x_1984_ = lean_box(0);
v_isShared_1985_ = v_isSharedCheck_1992_;
goto v_resetjp_1983_;
}
v_resetjp_1983_:
{
lean_object* v___x_1986_; lean_object* v___x_1987_; lean_object* v___x_1988_; lean_object* v___x_1990_; 
v___x_1986_ = lean_box(0);
lean_inc(v_same_1979_);
v___x_1987_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_Concl_restrict_spec__0(v_same_1979_, v_hs_1981_, v___x_1986_);
v___x_1988_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_Concl_restrict_spec__0(v_same_1979_, v_ps_1982_, v___x_1986_);
if (v_isShared_1985_ == 0)
{
lean_ctor_set(v___x_1984_, 1, v___x_1988_);
lean_ctor_set(v___x_1984_, 0, v___x_1987_);
v___x_1990_ = v___x_1984_;
goto v_reusejp_1989_;
}
else
{
lean_object* v_reuseFailAlloc_1991_; 
v_reuseFailAlloc_1991_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_1991_, 0, v___x_1987_);
lean_ctor_set(v_reuseFailAlloc_1991_, 1, v___x_1988_);
v___x_1990_ = v_reuseFailAlloc_1991_;
goto v_reusejp_1989_;
}
v_reusejp_1989_:
{
return v___x_1990_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_clobber_spec__0(lean_object* v_same_1993_, lean_object* v_a_1994_, lean_object* v_a_1995_){
_start:
{
if (lean_obj_tag(v_a_1994_) == 0)
{
lean_object* v___x_1996_; 
lean_dec(v_same_1993_);
v___x_1996_ = l_List_reverse___redArg(v_a_1995_);
return v___x_1996_;
}
else
{
lean_object* v_head_1997_; lean_object* v_tail_1998_; lean_object* v___x_2000_; uint8_t v_isShared_2001_; uint8_t v_isSharedCheck_2016_; 
v_head_1997_ = lean_ctor_get(v_a_1994_, 0);
v_tail_1998_ = lean_ctor_get(v_a_1994_, 1);
v_isSharedCheck_2016_ = !lean_is_exclusive(v_a_1994_);
if (v_isSharedCheck_2016_ == 0)
{
v___x_2000_ = v_a_1994_;
v_isShared_2001_ = v_isSharedCheck_2016_;
goto v_resetjp_1999_;
}
else
{
lean_inc(v_tail_1998_);
lean_inc(v_head_1997_);
lean_dec(v_a_1994_);
v___x_2000_ = lean_box(0);
v_isShared_2001_ = v_isSharedCheck_2016_;
goto v_resetjp_1999_;
}
v_resetjp_1999_:
{
lean_object* v_gs_2002_; lean_object* v_c_2003_; lean_object* v___x_2005_; uint8_t v_isShared_2006_; uint8_t v_isSharedCheck_2015_; 
v_gs_2002_ = lean_ctor_get(v_head_1997_, 0);
v_c_2003_ = lean_ctor_get(v_head_1997_, 1);
v_isSharedCheck_2015_ = !lean_is_exclusive(v_head_1997_);
if (v_isSharedCheck_2015_ == 0)
{
v___x_2005_ = v_head_1997_;
v_isShared_2006_ = v_isSharedCheck_2015_;
goto v_resetjp_2004_;
}
else
{
lean_inc(v_c_2003_);
lean_inc(v_gs_2002_);
lean_dec(v_head_1997_);
v___x_2005_ = lean_box(0);
v_isShared_2006_ = v_isSharedCheck_2015_;
goto v_resetjp_2004_;
}
v_resetjp_2004_:
{
lean_object* v___x_2007_; lean_object* v___x_2009_; 
lean_inc(v_same_1993_);
v___x_2007_ = lp_clauseflow_ClauseFlow_Concl_restrict(v_same_1993_, v_c_2003_);
if (v_isShared_2006_ == 0)
{
lean_ctor_set(v___x_2005_, 1, v___x_2007_);
v___x_2009_ = v___x_2005_;
goto v_reusejp_2008_;
}
else
{
lean_object* v_reuseFailAlloc_2014_; 
v_reuseFailAlloc_2014_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2014_, 0, v_gs_2002_);
lean_ctor_set(v_reuseFailAlloc_2014_, 1, v___x_2007_);
v___x_2009_ = v_reuseFailAlloc_2014_;
goto v_reusejp_2008_;
}
v_reusejp_2008_:
{
lean_object* v___x_2011_; 
if (v_isShared_2001_ == 0)
{
lean_ctor_set(v___x_2000_, 1, v_a_1995_);
lean_ctor_set(v___x_2000_, 0, v___x_2009_);
v___x_2011_ = v___x_2000_;
goto v_reusejp_2010_;
}
else
{
lean_object* v_reuseFailAlloc_2013_; 
v_reuseFailAlloc_2013_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2013_, 0, v___x_2009_);
lean_ctor_set(v_reuseFailAlloc_2013_, 1, v_a_1995_);
v___x_2011_ = v_reuseFailAlloc_2013_;
goto v_reusejp_2010_;
}
v_reusejp_2010_:
{
v_a_1994_ = v_tail_1998_;
v_a_1995_ = v___x_2011_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_clobber(lean_object* v_same_2017_, lean_object* v_a_2018_){
_start:
{
lean_object* v_facts_2019_; lean_object* v_cls_2020_; lean_object* v___x_2022_; uint8_t v_isShared_2023_; uint8_t v_isSharedCheck_2029_; 
v_facts_2019_ = lean_ctor_get(v_a_2018_, 0);
v_cls_2020_ = lean_ctor_get(v_a_2018_, 1);
v_isSharedCheck_2029_ = !lean_is_exclusive(v_a_2018_);
if (v_isSharedCheck_2029_ == 0)
{
v___x_2022_ = v_a_2018_;
v_isShared_2023_ = v_isSharedCheck_2029_;
goto v_resetjp_2021_;
}
else
{
lean_inc(v_cls_2020_);
lean_inc(v_facts_2019_);
lean_dec(v_a_2018_);
v___x_2022_ = lean_box(0);
v_isShared_2023_ = v_isSharedCheck_2029_;
goto v_resetjp_2021_;
}
v_resetjp_2021_:
{
lean_object* v___x_2024_; lean_object* v___x_2025_; lean_object* v___x_2027_; 
v___x_2024_ = lean_box(0);
v___x_2025_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_clobber_spec__0(v_same_2017_, v_cls_2020_, v___x_2024_);
if (v_isShared_2023_ == 0)
{
lean_ctor_set(v___x_2022_, 1, v___x_2025_);
v___x_2027_ = v___x_2022_;
goto v_reusejp_2026_;
}
else
{
lean_object* v_reuseFailAlloc_2028_; 
v_reuseFailAlloc_2028_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2028_, 0, v_facts_2019_);
lean_ctor_set(v_reuseFailAlloc_2028_, 1, v___x_2025_);
v___x_2027_ = v_reuseFailAlloc_2028_;
goto v_reusejp_2026_;
}
v_reusejp_2026_:
{
return v___x_2027_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameLit(lean_object* v_m_2030_, lean_object* v_l_2031_, lean_object* v_g_2032_){
_start:
{
lean_object* v_fst_2033_; lean_object* v_snd_2034_; uint8_t v___x_2035_; 
v_fst_2033_ = lean_ctor_get(v_g_2032_, 0);
v_snd_2034_ = lean_ctor_get(v_g_2032_, 1);
v___x_2035_ = lean_nat_dec_eq(v_fst_2033_, v_m_2030_);
if (v___x_2035_ == 0)
{
lean_dec(v_l_2031_);
return v_g_2032_;
}
else
{
lean_object* v___x_2037_; uint8_t v_isShared_2038_; uint8_t v_isSharedCheck_2042_; 
lean_inc(v_snd_2034_);
v_isSharedCheck_2042_ = !lean_is_exclusive(v_g_2032_);
if (v_isSharedCheck_2042_ == 0)
{
lean_object* v_unused_2043_; lean_object* v_unused_2044_; 
v_unused_2043_ = lean_ctor_get(v_g_2032_, 1);
lean_dec(v_unused_2043_);
v_unused_2044_ = lean_ctor_get(v_g_2032_, 0);
lean_dec(v_unused_2044_);
v___x_2037_ = v_g_2032_;
v_isShared_2038_ = v_isSharedCheck_2042_;
goto v_resetjp_2036_;
}
else
{
lean_dec(v_g_2032_);
v___x_2037_ = lean_box(0);
v_isShared_2038_ = v_isSharedCheck_2042_;
goto v_resetjp_2036_;
}
v_resetjp_2036_:
{
lean_object* v___x_2040_; 
if (v_isShared_2038_ == 0)
{
lean_ctor_set(v___x_2037_, 0, v_l_2031_);
v___x_2040_ = v___x_2037_;
goto v_reusejp_2039_;
}
else
{
lean_object* v_reuseFailAlloc_2041_; 
v_reuseFailAlloc_2041_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2041_, 0, v_l_2031_);
lean_ctor_set(v_reuseFailAlloc_2041_, 1, v_snd_2034_);
v___x_2040_ = v_reuseFailAlloc_2041_;
goto v_reusejp_2039_;
}
v_reusejp_2039_:
{
return v___x_2040_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameLit___boxed(lean_object* v_m_2045_, lean_object* v_l_2046_, lean_object* v_g_2047_){
_start:
{
lean_object* v_res_2048_; 
v_res_2048_ = lp_clauseflow_ClauseFlow_renameLit(v_m_2045_, v_l_2046_, v_g_2047_);
lean_dec(v_m_2045_);
return v_res_2048_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0(lean_object* v_m_2049_, lean_object* v_l_2050_, lean_object* v_a_2051_, lean_object* v_a_2052_){
_start:
{
if (lean_obj_tag(v_a_2051_) == 0)
{
lean_object* v___x_2053_; 
lean_dec(v_l_2050_);
v___x_2053_ = l_List_reverse___redArg(v_a_2052_);
return v___x_2053_;
}
else
{
lean_object* v_head_2054_; lean_object* v_tail_2055_; lean_object* v___x_2057_; uint8_t v_isShared_2058_; uint8_t v_isSharedCheck_2064_; 
v_head_2054_ = lean_ctor_get(v_a_2051_, 0);
v_tail_2055_ = lean_ctor_get(v_a_2051_, 1);
v_isSharedCheck_2064_ = !lean_is_exclusive(v_a_2051_);
if (v_isSharedCheck_2064_ == 0)
{
v___x_2057_ = v_a_2051_;
v_isShared_2058_ = v_isSharedCheck_2064_;
goto v_resetjp_2056_;
}
else
{
lean_inc(v_tail_2055_);
lean_inc(v_head_2054_);
lean_dec(v_a_2051_);
v___x_2057_ = lean_box(0);
v_isShared_2058_ = v_isSharedCheck_2064_;
goto v_resetjp_2056_;
}
v_resetjp_2056_:
{
lean_object* v___x_2059_; lean_object* v___x_2061_; 
lean_inc(v_l_2050_);
v___x_2059_ = lp_clauseflow_ClauseFlow_renameLit(v_m_2049_, v_l_2050_, v_head_2054_);
if (v_isShared_2058_ == 0)
{
lean_ctor_set(v___x_2057_, 1, v_a_2052_);
lean_ctor_set(v___x_2057_, 0, v___x_2059_);
v___x_2061_ = v___x_2057_;
goto v_reusejp_2060_;
}
else
{
lean_object* v_reuseFailAlloc_2063_; 
v_reuseFailAlloc_2063_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2063_, 0, v___x_2059_);
lean_ctor_set(v_reuseFailAlloc_2063_, 1, v_a_2052_);
v___x_2061_ = v_reuseFailAlloc_2063_;
goto v_reusejp_2060_;
}
v_reusejp_2060_:
{
v_a_2051_ = v_tail_2055_;
v_a_2052_ = v___x_2061_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0___boxed(lean_object* v_m_2065_, lean_object* v_l_2066_, lean_object* v_a_2067_, lean_object* v_a_2068_){
_start:
{
lean_object* v_res_2069_; 
v_res_2069_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0(v_m_2065_, v_l_2066_, v_a_2067_, v_a_2068_);
lean_dec(v_m_2065_);
return v_res_2069_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameCl(lean_object* v_m_2070_, lean_object* v_l_2071_, lean_object* v_c_2072_){
_start:
{
lean_object* v_gs_2073_; lean_object* v_c_2074_; lean_object* v___x_2076_; uint8_t v_isShared_2077_; uint8_t v_isSharedCheck_2083_; 
v_gs_2073_ = lean_ctor_get(v_c_2072_, 0);
v_c_2074_ = lean_ctor_get(v_c_2072_, 1);
v_isSharedCheck_2083_ = !lean_is_exclusive(v_c_2072_);
if (v_isSharedCheck_2083_ == 0)
{
v___x_2076_ = v_c_2072_;
v_isShared_2077_ = v_isSharedCheck_2083_;
goto v_resetjp_2075_;
}
else
{
lean_inc(v_c_2074_);
lean_inc(v_gs_2073_);
lean_dec(v_c_2072_);
v___x_2076_ = lean_box(0);
v_isShared_2077_ = v_isSharedCheck_2083_;
goto v_resetjp_2075_;
}
v_resetjp_2075_:
{
lean_object* v___x_2078_; lean_object* v___x_2079_; lean_object* v___x_2081_; 
v___x_2078_ = lean_box(0);
v___x_2079_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0(v_m_2070_, v_l_2071_, v_gs_2073_, v___x_2078_);
if (v_isShared_2077_ == 0)
{
lean_ctor_set(v___x_2076_, 0, v___x_2079_);
v___x_2081_ = v___x_2076_;
goto v_reusejp_2080_;
}
else
{
lean_object* v_reuseFailAlloc_2082_; 
v_reuseFailAlloc_2082_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2082_, 0, v___x_2079_);
lean_ctor_set(v_reuseFailAlloc_2082_, 1, v_c_2074_);
v___x_2081_ = v_reuseFailAlloc_2082_;
goto v_reusejp_2080_;
}
v_reusejp_2080_:
{
return v___x_2081_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_renameCl___boxed(lean_object* v_m_2084_, lean_object* v_l_2085_, lean_object* v_c_2086_){
_start:
{
lean_object* v_res_2087_; 
v_res_2087_ = lp_clauseflow_ClauseFlow_renameCl(v_m_2084_, v_l_2085_, v_c_2086_);
lean_dec(v_m_2084_);
return v_res_2087_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_swapLit(lean_object* v_m_2088_, lean_object* v_n_2089_, lean_object* v_l_2090_, lean_object* v_tEq_2091_, lean_object* v_tNe_2092_, lean_object* v_g_2093_){
_start:
{
lean_object* v___x_2094_; lean_object* v___x_2095_; lean_object* v___x_2096_; lean_object* v___x_2097_; uint8_t v___x_2098_; 
v___x_2094_ = lean_alloc_closure((void*)(l_instDecidableEqNat___boxed), 2, 0);
v___x_2095_ = lean_alloc_closure((void*)(lp_clauseflow_ClauseFlow_instDecidableEqTest___boxed), 2, 0);
lean_inc(v_n_2089_);
v___x_2096_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v___x_2096_, 0, v_n_2089_);
lean_inc(v_m_2088_);
v___x_2097_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2097_, 0, v_m_2088_);
lean_ctor_set(v___x_2097_, 1, v___x_2096_);
lean_inc_ref(v_g_2093_);
lean_inc_ref(v___x_2095_);
lean_inc_ref(v___x_2094_);
v___x_2098_ = l_instDecidableEqProd___redArg(v___x_2094_, v___x_2095_, v_g_2093_, v___x_2097_);
if (v___x_2098_ == 0)
{
lean_object* v___x_2099_; lean_object* v___x_2100_; uint8_t v___x_2101_; 
lean_dec_ref(v_tEq_2091_);
v___x_2099_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_2099_, 0, v_n_2089_);
v___x_2100_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2100_, 0, v_m_2088_);
lean_ctor_set(v___x_2100_, 1, v___x_2099_);
lean_inc_ref(v_g_2093_);
v___x_2101_ = l_instDecidableEqProd___redArg(v___x_2094_, v___x_2095_, v_g_2093_, v___x_2100_);
if (v___x_2101_ == 0)
{
lean_dec_ref(v_tNe_2092_);
lean_dec(v_l_2090_);
return v_g_2093_;
}
else
{
lean_object* v___x_2102_; 
lean_dec_ref(v_g_2093_);
v___x_2102_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2102_, 0, v_l_2090_);
lean_ctor_set(v___x_2102_, 1, v_tNe_2092_);
return v___x_2102_;
}
}
else
{
lean_object* v___x_2103_; 
lean_dec_ref(v___x_2095_);
lean_dec_ref(v___x_2094_);
lean_dec_ref(v_g_2093_);
lean_dec_ref(v_tNe_2092_);
lean_dec(v_n_2089_);
lean_dec(v_m_2088_);
v___x_2103_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2103_, 0, v_l_2090_);
lean_ctor_set(v___x_2103_, 1, v_tEq_2091_);
return v___x_2103_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_cmpCls_spec__0(lean_object* v_m_2104_, lean_object* v_n_2105_, lean_object* v_l_2106_, lean_object* v_tEq_2107_, lean_object* v_tNe_2108_, lean_object* v_a_2109_, lean_object* v_a_2110_){
_start:
{
if (lean_obj_tag(v_a_2109_) == 0)
{
lean_object* v___x_2111_; 
lean_dec_ref(v_tNe_2108_);
lean_dec_ref(v_tEq_2107_);
lean_dec(v_l_2106_);
lean_dec(v_n_2105_);
lean_dec(v_m_2104_);
v___x_2111_ = l_List_reverse___redArg(v_a_2110_);
return v___x_2111_;
}
else
{
lean_object* v_head_2112_; lean_object* v_tail_2113_; lean_object* v___x_2115_; uint8_t v_isShared_2116_; uint8_t v_isSharedCheck_2122_; 
v_head_2112_ = lean_ctor_get(v_a_2109_, 0);
v_tail_2113_ = lean_ctor_get(v_a_2109_, 1);
v_isSharedCheck_2122_ = !lean_is_exclusive(v_a_2109_);
if (v_isSharedCheck_2122_ == 0)
{
v___x_2115_ = v_a_2109_;
v_isShared_2116_ = v_isSharedCheck_2122_;
goto v_resetjp_2114_;
}
else
{
lean_inc(v_tail_2113_);
lean_inc(v_head_2112_);
lean_dec(v_a_2109_);
v___x_2115_ = lean_box(0);
v_isShared_2116_ = v_isSharedCheck_2122_;
goto v_resetjp_2114_;
}
v_resetjp_2114_:
{
lean_object* v___x_2117_; lean_object* v___x_2119_; 
lean_inc_ref(v_tNe_2108_);
lean_inc_ref(v_tEq_2107_);
lean_inc(v_l_2106_);
lean_inc(v_n_2105_);
lean_inc(v_m_2104_);
v___x_2117_ = lp_clauseflow_ClauseFlow_swapLit(v_m_2104_, v_n_2105_, v_l_2106_, v_tEq_2107_, v_tNe_2108_, v_head_2112_);
if (v_isShared_2116_ == 0)
{
lean_ctor_set(v___x_2115_, 1, v_a_2110_);
lean_ctor_set(v___x_2115_, 0, v___x_2117_);
v___x_2119_ = v___x_2115_;
goto v_reusejp_2118_;
}
else
{
lean_object* v_reuseFailAlloc_2121_; 
v_reuseFailAlloc_2121_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2121_, 0, v___x_2117_);
lean_ctor_set(v_reuseFailAlloc_2121_, 1, v_a_2110_);
v___x_2119_ = v_reuseFailAlloc_2121_;
goto v_reusejp_2118_;
}
v_reusejp_2118_:
{
v_a_2109_ = v_tail_2113_;
v_a_2110_ = v___x_2119_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_cmpCls_spec__2(lean_object* v_m_2123_, lean_object* v_n_2124_, lean_object* v_l_2125_, lean_object* v_tEq_2126_, lean_object* v_tNe_2127_, lean_object* v_a_2128_, lean_object* v_a_2129_){
_start:
{
if (lean_obj_tag(v_a_2128_) == 0)
{
lean_object* v___x_2130_; 
lean_dec_ref(v_tNe_2127_);
lean_dec_ref(v_tEq_2126_);
lean_dec(v_l_2125_);
lean_dec(v_n_2124_);
lean_dec(v_m_2123_);
v___x_2130_ = l_List_reverse___redArg(v_a_2129_);
return v___x_2130_;
}
else
{
lean_object* v_head_2131_; lean_object* v_tail_2132_; lean_object* v___x_2134_; uint8_t v_isShared_2135_; uint8_t v_isSharedCheck_2151_; 
v_head_2131_ = lean_ctor_get(v_a_2128_, 0);
v_tail_2132_ = lean_ctor_get(v_a_2128_, 1);
v_isSharedCheck_2151_ = !lean_is_exclusive(v_a_2128_);
if (v_isSharedCheck_2151_ == 0)
{
v___x_2134_ = v_a_2128_;
v_isShared_2135_ = v_isSharedCheck_2151_;
goto v_resetjp_2133_;
}
else
{
lean_inc(v_tail_2132_);
lean_inc(v_head_2131_);
lean_dec(v_a_2128_);
v___x_2134_ = lean_box(0);
v_isShared_2135_ = v_isSharedCheck_2151_;
goto v_resetjp_2133_;
}
v_resetjp_2133_:
{
lean_object* v_gs_2136_; lean_object* v_c_2137_; lean_object* v___x_2139_; uint8_t v_isShared_2140_; uint8_t v_isSharedCheck_2150_; 
v_gs_2136_ = lean_ctor_get(v_head_2131_, 0);
v_c_2137_ = lean_ctor_get(v_head_2131_, 1);
v_isSharedCheck_2150_ = !lean_is_exclusive(v_head_2131_);
if (v_isSharedCheck_2150_ == 0)
{
v___x_2139_ = v_head_2131_;
v_isShared_2140_ = v_isSharedCheck_2150_;
goto v_resetjp_2138_;
}
else
{
lean_inc(v_c_2137_);
lean_inc(v_gs_2136_);
lean_dec(v_head_2131_);
v___x_2139_ = lean_box(0);
v_isShared_2140_ = v_isSharedCheck_2150_;
goto v_resetjp_2138_;
}
v_resetjp_2138_:
{
lean_object* v___x_2141_; lean_object* v___x_2142_; lean_object* v___x_2144_; 
v___x_2141_ = lean_box(0);
lean_inc_ref(v_tNe_2127_);
lean_inc_ref(v_tEq_2126_);
lean_inc(v_l_2125_);
lean_inc(v_n_2124_);
lean_inc(v_m_2123_);
v___x_2142_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_cmpCls_spec__0(v_m_2123_, v_n_2124_, v_l_2125_, v_tEq_2126_, v_tNe_2127_, v_gs_2136_, v___x_2141_);
if (v_isShared_2140_ == 0)
{
lean_ctor_set(v___x_2139_, 0, v___x_2142_);
v___x_2144_ = v___x_2139_;
goto v_reusejp_2143_;
}
else
{
lean_object* v_reuseFailAlloc_2149_; 
v_reuseFailAlloc_2149_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2149_, 0, v___x_2142_);
lean_ctor_set(v_reuseFailAlloc_2149_, 1, v_c_2137_);
v___x_2144_ = v_reuseFailAlloc_2149_;
goto v_reusejp_2143_;
}
v_reusejp_2143_:
{
lean_object* v___x_2146_; 
if (v_isShared_2135_ == 0)
{
lean_ctor_set(v___x_2134_, 1, v_a_2129_);
lean_ctor_set(v___x_2134_, 0, v___x_2144_);
v___x_2146_ = v___x_2134_;
goto v_reusejp_2145_;
}
else
{
lean_object* v_reuseFailAlloc_2148_; 
v_reuseFailAlloc_2148_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2148_, 0, v___x_2144_);
lean_ctor_set(v_reuseFailAlloc_2148_, 1, v_a_2129_);
v___x_2146_ = v_reuseFailAlloc_2148_;
goto v_reusejp_2145_;
}
v_reusejp_2145_:
{
v_a_2128_ = v_tail_2132_;
v_a_2129_ = v___x_2146_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1(lean_object* v_m_2152_, lean_object* v_a_2153_, lean_object* v_a_2154_){
_start:
{
if (lean_obj_tag(v_a_2153_) == 0)
{
lean_object* v___x_2155_; 
v___x_2155_ = l_List_reverse___redArg(v_a_2154_);
return v___x_2155_;
}
else
{
lean_object* v_head_2156_; lean_object* v_tail_2157_; lean_object* v___x_2159_; uint8_t v_isShared_2160_; uint8_t v_isSharedCheck_2167_; 
v_head_2156_ = lean_ctor_get(v_a_2153_, 0);
v_tail_2157_ = lean_ctor_get(v_a_2153_, 1);
v_isSharedCheck_2167_ = !lean_is_exclusive(v_a_2153_);
if (v_isSharedCheck_2167_ == 0)
{
v___x_2159_ = v_a_2153_;
v_isShared_2160_ = v_isSharedCheck_2167_;
goto v_resetjp_2158_;
}
else
{
lean_inc(v_tail_2157_);
lean_inc(v_head_2156_);
lean_dec(v_a_2153_);
v___x_2159_ = lean_box(0);
v_isShared_2160_ = v_isSharedCheck_2167_;
goto v_resetjp_2158_;
}
v_resetjp_2158_:
{
uint8_t v___x_2161_; 
v___x_2161_ = lp_clauseflow_ClauseFlow_mentions(v_m_2152_, v_head_2156_);
if (v___x_2161_ == 0)
{
lean_del_object(v___x_2159_);
lean_dec(v_head_2156_);
v_a_2153_ = v_tail_2157_;
goto _start;
}
else
{
lean_object* v___x_2164_; 
if (v_isShared_2160_ == 0)
{
lean_ctor_set(v___x_2159_, 1, v_a_2154_);
v___x_2164_ = v___x_2159_;
goto v_reusejp_2163_;
}
else
{
lean_object* v_reuseFailAlloc_2166_; 
v_reuseFailAlloc_2166_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2166_, 0, v_head_2156_);
lean_ctor_set(v_reuseFailAlloc_2166_, 1, v_a_2154_);
v___x_2164_ = v_reuseFailAlloc_2166_;
goto v_reusejp_2163_;
}
v_reusejp_2163_:
{
v_a_2153_ = v_tail_2157_;
v_a_2154_ = v___x_2164_;
goto _start;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1___boxed(lean_object* v_m_2168_, lean_object* v_a_2169_, lean_object* v_a_2170_){
_start:
{
lean_object* v_res_2171_; 
v_res_2171_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1(v_m_2168_, v_a_2169_, v_a_2170_);
lean_dec(v_m_2168_);
return v_res_2171_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_cmpCls(lean_object* v_a_2172_, lean_object* v_m_2173_, lean_object* v_n_2174_, lean_object* v_l_2175_, lean_object* v_tEq_2176_, lean_object* v_tNe_2177_){
_start:
{
lean_object* v_cls_2178_; lean_object* v___x_2179_; lean_object* v___x_2180_; lean_object* v___x_2181_; lean_object* v___x_2182_; 
v_cls_2178_ = lean_ctor_get(v_a_2172_, 1);
lean_inc_n(v_cls_2178_, 2);
lean_dec_ref(v_a_2172_);
v___x_2179_ = lean_box(0);
v___x_2180_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1(v_m_2173_, v_cls_2178_, v___x_2179_);
v___x_2181_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_cmpCls_spec__2(v_m_2173_, v_n_2174_, v_l_2175_, v_tEq_2176_, v_tNe_2177_, v___x_2180_, v___x_2179_);
v___x_2182_ = l_List_appendTR___redArg(v___x_2181_, v_cls_2178_);
return v___x_2182_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__2(lean_object* v_m_2183_, lean_object* v_l_2184_, lean_object* v_a_2185_, lean_object* v_a_2186_){
_start:
{
if (lean_obj_tag(v_a_2185_) == 0)
{
lean_object* v___x_2187_; 
lean_dec(v_l_2184_);
v___x_2187_ = l_List_reverse___redArg(v_a_2186_);
return v___x_2187_;
}
else
{
lean_object* v_head_2188_; lean_object* v_tail_2189_; lean_object* v___x_2191_; uint8_t v_isShared_2192_; uint8_t v_isSharedCheck_2198_; 
v_head_2188_ = lean_ctor_get(v_a_2185_, 0);
v_tail_2189_ = lean_ctor_get(v_a_2185_, 1);
v_isSharedCheck_2198_ = !lean_is_exclusive(v_a_2185_);
if (v_isSharedCheck_2198_ == 0)
{
v___x_2191_ = v_a_2185_;
v_isShared_2192_ = v_isSharedCheck_2198_;
goto v_resetjp_2190_;
}
else
{
lean_inc(v_tail_2189_);
lean_inc(v_head_2188_);
lean_dec(v_a_2185_);
v___x_2191_ = lean_box(0);
v_isShared_2192_ = v_isSharedCheck_2198_;
goto v_resetjp_2190_;
}
v_resetjp_2190_:
{
lean_object* v___x_2193_; lean_object* v___x_2195_; 
lean_inc(v_l_2184_);
v___x_2193_ = lp_clauseflow_ClauseFlow_renameCl(v_m_2183_, v_l_2184_, v_head_2188_);
if (v_isShared_2192_ == 0)
{
lean_ctor_set(v___x_2191_, 1, v_a_2186_);
lean_ctor_set(v___x_2191_, 0, v___x_2193_);
v___x_2195_ = v___x_2191_;
goto v_reusejp_2194_;
}
else
{
lean_object* v_reuseFailAlloc_2197_; 
v_reuseFailAlloc_2197_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2197_, 0, v___x_2193_);
lean_ctor_set(v_reuseFailAlloc_2197_, 1, v_a_2186_);
v___x_2195_ = v_reuseFailAlloc_2197_;
goto v_reusejp_2194_;
}
v_reusejp_2194_:
{
v_a_2185_ = v_tail_2189_;
v_a_2186_ = v___x_2195_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__2___boxed(lean_object* v_m_2199_, lean_object* v_l_2200_, lean_object* v_a_2201_, lean_object* v_a_2202_){
_start:
{
lean_object* v_res_2203_; 
v_res_2203_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__2(v_m_2199_, v_l_2200_, v_a_2201_, v_a_2202_);
lean_dec(v_m_2199_);
return v_res_2203_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tStmt_spec__0(lean_object* v_m_2204_, lean_object* v_a_2205_, lean_object* v_a_2206_){
_start:
{
if (lean_obj_tag(v_a_2205_) == 0)
{
lean_object* v___x_2207_; 
v___x_2207_ = l_List_reverse___redArg(v_a_2206_);
return v___x_2207_;
}
else
{
lean_object* v_head_2208_; lean_object* v_tail_2209_; lean_object* v___x_2211_; uint8_t v_isShared_2212_; uint8_t v_isSharedCheck_2220_; 
v_head_2208_ = lean_ctor_get(v_a_2205_, 0);
v_tail_2209_ = lean_ctor_get(v_a_2205_, 1);
v_isSharedCheck_2220_ = !lean_is_exclusive(v_a_2205_);
if (v_isSharedCheck_2220_ == 0)
{
v___x_2211_ = v_a_2205_;
v_isShared_2212_ = v_isSharedCheck_2220_;
goto v_resetjp_2210_;
}
else
{
lean_inc(v_tail_2209_);
lean_inc(v_head_2208_);
lean_dec(v_a_2205_);
v___x_2211_ = lean_box(0);
v_isShared_2212_ = v_isSharedCheck_2220_;
goto v_resetjp_2210_;
}
v_resetjp_2210_:
{
lean_object* v_fst_2213_; uint8_t v___x_2214_; 
v_fst_2213_ = lean_ctor_get(v_head_2208_, 0);
v___x_2214_ = lean_nat_dec_eq(v_fst_2213_, v_m_2204_);
if (v___x_2214_ == 0)
{
lean_del_object(v___x_2211_);
lean_dec(v_head_2208_);
v_a_2205_ = v_tail_2209_;
goto _start;
}
else
{
lean_object* v___x_2217_; 
if (v_isShared_2212_ == 0)
{
lean_ctor_set(v___x_2211_, 1, v_a_2206_);
v___x_2217_ = v___x_2211_;
goto v_reusejp_2216_;
}
else
{
lean_object* v_reuseFailAlloc_2219_; 
v_reuseFailAlloc_2219_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2219_, 0, v_head_2208_);
lean_ctor_set(v_reuseFailAlloc_2219_, 1, v_a_2206_);
v___x_2217_ = v_reuseFailAlloc_2219_;
goto v_reusejp_2216_;
}
v_reusejp_2216_:
{
v_a_2205_ = v_tail_2209_;
v_a_2206_ = v___x_2217_;
goto _start;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tStmt_spec__0___boxed(lean_object* v_m_2221_, lean_object* v_a_2222_, lean_object* v_a_2223_){
_start:
{
lean_object* v_res_2224_; 
v_res_2224_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tStmt_spec__0(v_m_2221_, v_a_2222_, v_a_2223_);
lean_dec(v_m_2221_);
return v_res_2224_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__1(lean_object* v_l_2225_, lean_object* v_a_2226_, lean_object* v_a_2227_){
_start:
{
if (lean_obj_tag(v_a_2226_) == 0)
{
lean_object* v___x_2228_; 
lean_dec(v_l_2225_);
v___x_2228_ = l_List_reverse___redArg(v_a_2227_);
return v___x_2228_;
}
else
{
lean_object* v_head_2229_; lean_object* v_tail_2230_; lean_object* v___x_2232_; uint8_t v_isShared_2233_; uint8_t v_isSharedCheck_2247_; 
v_head_2229_ = lean_ctor_get(v_a_2226_, 0);
v_tail_2230_ = lean_ctor_get(v_a_2226_, 1);
v_isSharedCheck_2247_ = !lean_is_exclusive(v_a_2226_);
if (v_isSharedCheck_2247_ == 0)
{
v___x_2232_ = v_a_2226_;
v_isShared_2233_ = v_isSharedCheck_2247_;
goto v_resetjp_2231_;
}
else
{
lean_inc(v_tail_2230_);
lean_inc(v_head_2229_);
lean_dec(v_a_2226_);
v___x_2232_ = lean_box(0);
v_isShared_2233_ = v_isSharedCheck_2247_;
goto v_resetjp_2231_;
}
v_resetjp_2231_:
{
lean_object* v_snd_2234_; lean_object* v___x_2236_; uint8_t v_isShared_2237_; uint8_t v_isSharedCheck_2245_; 
v_snd_2234_ = lean_ctor_get(v_head_2229_, 1);
v_isSharedCheck_2245_ = !lean_is_exclusive(v_head_2229_);
if (v_isSharedCheck_2245_ == 0)
{
lean_object* v_unused_2246_; 
v_unused_2246_ = lean_ctor_get(v_head_2229_, 0);
lean_dec(v_unused_2246_);
v___x_2236_ = v_head_2229_;
v_isShared_2237_ = v_isSharedCheck_2245_;
goto v_resetjp_2235_;
}
else
{
lean_inc(v_snd_2234_);
lean_dec(v_head_2229_);
v___x_2236_ = lean_box(0);
v_isShared_2237_ = v_isSharedCheck_2245_;
goto v_resetjp_2235_;
}
v_resetjp_2235_:
{
lean_object* v___x_2239_; 
lean_inc(v_l_2225_);
if (v_isShared_2237_ == 0)
{
lean_ctor_set(v___x_2236_, 0, v_l_2225_);
v___x_2239_ = v___x_2236_;
goto v_reusejp_2238_;
}
else
{
lean_object* v_reuseFailAlloc_2244_; 
v_reuseFailAlloc_2244_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2244_, 0, v_l_2225_);
lean_ctor_set(v_reuseFailAlloc_2244_, 1, v_snd_2234_);
v___x_2239_ = v_reuseFailAlloc_2244_;
goto v_reusejp_2238_;
}
v_reusejp_2238_:
{
lean_object* v___x_2241_; 
if (v_isShared_2233_ == 0)
{
lean_ctor_set(v___x_2232_, 1, v_a_2227_);
lean_ctor_set(v___x_2232_, 0, v___x_2239_);
v___x_2241_ = v___x_2232_;
goto v_reusejp_2240_;
}
else
{
lean_object* v_reuseFailAlloc_2243_; 
v_reuseFailAlloc_2243_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2243_, 0, v___x_2239_);
lean_ctor_set(v_reuseFailAlloc_2243_, 1, v_a_2227_);
v___x_2241_ = v_reuseFailAlloc_2243_;
goto v_reusejp_2240_;
}
v_reusejp_2240_:
{
v_a_2226_ = v_tail_2230_;
v_a_2227_ = v___x_2241_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_tStmt(lean_object* v_x_2252_, lean_object* v_x_2253_){
_start:
{
switch(lean_obj_tag(v_x_2252_))
{
case 0:
{
lean_object* v_l_2254_; lean_object* v___x_2255_; 
v_l_2254_ = lean_ctor_get(v_x_2252_, 0);
lean_inc(v_l_2254_);
lean_dec_ref_known(v_x_2252_, 1);
v___x_2255_ = lp_clauseflow_ClauseFlow_kill(v_l_2254_, v_x_2253_);
lean_dec(v_l_2254_);
return v___x_2255_;
}
case 1:
{
lean_object* v_same_2256_; lean_object* v___x_2257_; 
v_same_2256_ = lean_ctor_get(v_x_2252_, 0);
lean_inc(v_same_2256_);
lean_dec_ref_known(v_x_2252_, 1);
v___x_2257_ = lp_clauseflow_ClauseFlow_clobber(v_same_2256_, v_x_2253_);
return v___x_2257_;
}
case 2:
{
lean_object* v_l_2258_; lean_object* v_d_2259_; lean_object* v___x_2261_; uint8_t v_isShared_2262_; uint8_t v_isSharedCheck_2278_; 
v_l_2258_ = lean_ctor_get(v_x_2252_, 0);
v_d_2259_ = lean_ctor_get(v_x_2252_, 1);
v_isSharedCheck_2278_ = !lean_is_exclusive(v_x_2252_);
if (v_isSharedCheck_2278_ == 0)
{
v___x_2261_ = v_x_2252_;
v_isShared_2262_ = v_isSharedCheck_2278_;
goto v_resetjp_2260_;
}
else
{
lean_inc(v_d_2259_);
lean_inc(v_l_2258_);
lean_dec(v_x_2252_);
v___x_2261_ = lean_box(0);
v_isShared_2262_ = v_isSharedCheck_2278_;
goto v_resetjp_2260_;
}
v_resetjp_2260_:
{
lean_object* v_a1_2263_; lean_object* v_facts_2264_; lean_object* v_cls_2265_; lean_object* v___x_2267_; uint8_t v_isShared_2268_; uint8_t v_isSharedCheck_2277_; 
v_a1_2263_ = lp_clauseflow_ClauseFlow_kill(v_l_2258_, v_x_2253_);
v_facts_2264_ = lean_ctor_get(v_a1_2263_, 0);
v_cls_2265_ = lean_ctor_get(v_a1_2263_, 1);
v_isSharedCheck_2277_ = !lean_is_exclusive(v_a1_2263_);
if (v_isSharedCheck_2277_ == 0)
{
v___x_2267_ = v_a1_2263_;
v_isShared_2268_ = v_isSharedCheck_2277_;
goto v_resetjp_2266_;
}
else
{
lean_inc(v_cls_2265_);
lean_inc(v_facts_2264_);
lean_dec(v_a1_2263_);
v___x_2267_ = lean_box(0);
v_isShared_2268_ = v_isSharedCheck_2277_;
goto v_resetjp_2266_;
}
v_resetjp_2266_:
{
lean_object* v___x_2269_; lean_object* v___x_2271_; 
v___x_2269_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v___x_2269_, 0, v_d_2259_);
if (v_isShared_2262_ == 0)
{
lean_ctor_set_tag(v___x_2261_, 0);
lean_ctor_set(v___x_2261_, 1, v___x_2269_);
v___x_2271_ = v___x_2261_;
goto v_reusejp_2270_;
}
else
{
lean_object* v_reuseFailAlloc_2276_; 
v_reuseFailAlloc_2276_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2276_, 0, v_l_2258_);
lean_ctor_set(v_reuseFailAlloc_2276_, 1, v___x_2269_);
v___x_2271_ = v_reuseFailAlloc_2276_;
goto v_reusejp_2270_;
}
v_reusejp_2270_:
{
lean_object* v___x_2272_; lean_object* v___x_2274_; 
v___x_2272_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2272_, 0, v___x_2271_);
lean_ctor_set(v___x_2272_, 1, v_facts_2264_);
if (v_isShared_2268_ == 0)
{
lean_ctor_set(v___x_2267_, 0, v___x_2272_);
v___x_2274_ = v___x_2267_;
goto v_reusejp_2273_;
}
else
{
lean_object* v_reuseFailAlloc_2275_; 
v_reuseFailAlloc_2275_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2275_, 0, v___x_2272_);
lean_ctor_set(v_reuseFailAlloc_2275_, 1, v_cls_2265_);
v___x_2274_ = v_reuseFailAlloc_2275_;
goto v_reusejp_2273_;
}
v_reusejp_2273_:
{
return v___x_2274_;
}
}
}
}
}
case 3:
{
lean_object* v_l_2279_; lean_object* v_m_2280_; uint8_t v___x_2281_; 
v_l_2279_ = lean_ctor_get(v_x_2252_, 0);
lean_inc(v_l_2279_);
v_m_2280_ = lean_ctor_get(v_x_2252_, 1);
lean_inc(v_m_2280_);
lean_dec_ref_known(v_x_2252_, 2);
v___x_2281_ = lean_nat_dec_eq(v_l_2279_, v_m_2280_);
if (v___x_2281_ == 0)
{
lean_object* v_a1_2282_; lean_object* v_facts_2283_; lean_object* v_cls_2284_; lean_object* v___x_2286_; uint8_t v_isShared_2287_; uint8_t v_isSharedCheck_2298_; 
v_a1_2282_ = lp_clauseflow_ClauseFlow_kill(v_l_2279_, v_x_2253_);
v_facts_2283_ = lean_ctor_get(v_a1_2282_, 0);
v_cls_2284_ = lean_ctor_get(v_a1_2282_, 1);
v_isSharedCheck_2298_ = !lean_is_exclusive(v_a1_2282_);
if (v_isSharedCheck_2298_ == 0)
{
v___x_2286_ = v_a1_2282_;
v_isShared_2287_ = v_isSharedCheck_2298_;
goto v_resetjp_2285_;
}
else
{
lean_inc(v_cls_2284_);
lean_inc(v_facts_2283_);
lean_dec(v_a1_2282_);
v___x_2286_ = lean_box(0);
v_isShared_2287_ = v_isSharedCheck_2298_;
goto v_resetjp_2285_;
}
v_resetjp_2285_:
{
lean_object* v___x_2288_; lean_object* v___x_2289_; lean_object* v___x_2290_; lean_object* v___x_2291_; lean_object* v___x_2292_; lean_object* v___x_2293_; lean_object* v___x_2294_; lean_object* v___x_2296_; 
v___x_2288_ = lean_box(0);
lean_inc(v_facts_2283_);
v___x_2289_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tStmt_spec__0(v_m_2280_, v_facts_2283_, v___x_2288_);
lean_inc(v_l_2279_);
v___x_2290_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__1(v_l_2279_, v___x_2289_, v___x_2288_);
v___x_2291_ = l_List_appendTR___redArg(v___x_2290_, v_facts_2283_);
lean_inc(v_cls_2284_);
v___x_2292_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_cmpCls_spec__1(v_m_2280_, v_cls_2284_, v___x_2288_);
v___x_2293_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tStmt_spec__2(v_m_2280_, v_l_2279_, v___x_2292_, v___x_2288_);
lean_dec(v_m_2280_);
v___x_2294_ = l_List_appendTR___redArg(v___x_2293_, v_cls_2284_);
if (v_isShared_2287_ == 0)
{
lean_ctor_set(v___x_2286_, 1, v___x_2294_);
lean_ctor_set(v___x_2286_, 0, v___x_2291_);
v___x_2296_ = v___x_2286_;
goto v_reusejp_2295_;
}
else
{
lean_object* v_reuseFailAlloc_2297_; 
v_reuseFailAlloc_2297_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2297_, 0, v___x_2291_);
lean_ctor_set(v_reuseFailAlloc_2297_, 1, v___x_2294_);
v___x_2296_ = v_reuseFailAlloc_2297_;
goto v_reusejp_2295_;
}
v_reusejp_2295_:
{
return v___x_2296_;
}
}
}
else
{
lean_dec(v_m_2280_);
lean_dec(v_l_2279_);
return v_x_2253_;
}
}
case 4:
{
lean_object* v_l_2299_; lean_object* v_m_2300_; lean_object* v_n_2301_; uint8_t v___x_2302_; 
v_l_2299_ = lean_ctor_get(v_x_2252_, 0);
lean_inc(v_l_2299_);
v_m_2300_ = lean_ctor_get(v_x_2252_, 1);
lean_inc(v_m_2300_);
v_n_2301_ = lean_ctor_get(v_x_2252_, 2);
lean_inc(v_n_2301_);
lean_dec_ref_known(v_x_2252_, 3);
v___x_2302_ = lean_nat_dec_eq(v_l_2299_, v_m_2300_);
if (v___x_2302_ == 0)
{
lean_object* v_a1_2303_; lean_object* v_facts_2304_; lean_object* v___x_2305_; lean_object* v___x_2306_; lean_object* v___x_2307_; lean_object* v___x_2308_; lean_object* v___x_2309_; lean_object* v___x_2310_; lean_object* v___x_2311_; lean_object* v___x_2312_; lean_object* v___x_2313_; lean_object* v___x_2314_; lean_object* v___x_2315_; lean_object* v___x_2316_; lean_object* v___x_2317_; lean_object* v___x_2318_; lean_object* v___x_2319_; lean_object* v___x_2320_; lean_object* v___x_2321_; lean_object* v___x_2322_; lean_object* v___x_2323_; lean_object* v___x_2324_; 
v_a1_2303_ = lp_clauseflow_ClauseFlow_kill(v_l_2299_, v_x_2253_);
v_facts_2304_ = lean_ctor_get(v_a1_2303_, 0);
lean_inc(v_facts_2304_);
v___x_2305_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__0));
lean_inc_n(v_l_2299_, 2);
v___x_2306_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2306_, 0, v_l_2299_);
lean_ctor_set(v___x_2306_, 1, v___x_2305_);
lean_inc_n(v_n_2301_, 2);
v___x_2307_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v___x_2307_, 0, v_n_2301_);
lean_inc_n(v_m_2300_, 2);
v___x_2308_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2308_, 0, v_m_2300_);
lean_ctor_set(v___x_2308_, 1, v___x_2307_);
v___x_2309_ = lean_box(0);
v___x_2310_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2310_, 0, v___x_2308_);
lean_ctor_set(v___x_2310_, 1, v___x_2309_);
v___x_2311_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2311_, 0, v___x_2306_);
lean_ctor_set(v___x_2311_, 1, v___x_2310_);
v___x_2312_ = lean_box(0);
v___x_2313_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2313_, 0, v___x_2311_);
lean_ctor_set(v___x_2313_, 1, v___x_2312_);
v___x_2314_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__1));
v___x_2315_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2315_, 0, v_l_2299_);
lean_ctor_set(v___x_2315_, 1, v___x_2314_);
v___x_2316_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_2316_, 0, v_n_2301_);
v___x_2317_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2317_, 0, v_m_2300_);
lean_ctor_set(v___x_2317_, 1, v___x_2316_);
v___x_2318_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2318_, 0, v___x_2317_);
lean_ctor_set(v___x_2318_, 1, v___x_2309_);
v___x_2319_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2319_, 0, v___x_2315_);
lean_ctor_set(v___x_2319_, 1, v___x_2318_);
v___x_2320_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2320_, 0, v___x_2319_);
lean_ctor_set(v___x_2320_, 1, v___x_2312_);
v___x_2321_ = lp_clauseflow_ClauseFlow_cmpCls(v_a1_2303_, v_m_2300_, v_n_2301_, v_l_2299_, v___x_2314_, v___x_2305_);
v___x_2322_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2322_, 0, v___x_2320_);
lean_ctor_set(v___x_2322_, 1, v___x_2321_);
v___x_2323_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2323_, 0, v___x_2313_);
lean_ctor_set(v___x_2323_, 1, v___x_2322_);
v___x_2324_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2324_, 0, v_facts_2304_);
lean_ctor_set(v___x_2324_, 1, v___x_2323_);
return v___x_2324_;
}
else
{
lean_object* v___x_2325_; 
lean_dec(v_n_2301_);
lean_dec(v_m_2300_);
v___x_2325_ = lp_clauseflow_ClauseFlow_kill(v_l_2299_, v_x_2253_);
lean_dec(v_l_2299_);
return v___x_2325_;
}
}
default: 
{
lean_object* v_l_2326_; lean_object* v_m_2327_; lean_object* v_n_2328_; uint8_t v___x_2329_; 
v_l_2326_ = lean_ctor_get(v_x_2252_, 0);
lean_inc(v_l_2326_);
v_m_2327_ = lean_ctor_get(v_x_2252_, 1);
lean_inc(v_m_2327_);
v_n_2328_ = lean_ctor_get(v_x_2252_, 2);
lean_inc(v_n_2328_);
lean_dec_ref_known(v_x_2252_, 3);
v___x_2329_ = lean_nat_dec_eq(v_l_2326_, v_m_2327_);
if (v___x_2329_ == 0)
{
lean_object* v_a1_2330_; lean_object* v_facts_2331_; lean_object* v___x_2332_; lean_object* v___x_2333_; lean_object* v___x_2334_; lean_object* v___x_2335_; lean_object* v___x_2336_; lean_object* v___x_2337_; lean_object* v___x_2338_; lean_object* v___x_2339_; lean_object* v___x_2340_; lean_object* v___x_2341_; lean_object* v___x_2342_; lean_object* v___x_2343_; lean_object* v___x_2344_; lean_object* v___x_2345_; lean_object* v___x_2346_; lean_object* v___x_2347_; lean_object* v___x_2348_; lean_object* v___x_2349_; lean_object* v___x_2350_; lean_object* v___x_2351_; 
v_a1_2330_ = lp_clauseflow_ClauseFlow_kill(v_l_2326_, v_x_2253_);
v_facts_2331_ = lean_ctor_get(v_a1_2330_, 0);
lean_inc(v_facts_2331_);
v___x_2332_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__1));
lean_inc_n(v_l_2326_, 2);
v___x_2333_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2333_, 0, v_l_2326_);
lean_ctor_set(v___x_2333_, 1, v___x_2332_);
lean_inc_n(v_n_2328_, 2);
v___x_2334_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v___x_2334_, 0, v_n_2328_);
lean_inc_n(v_m_2327_, 2);
v___x_2335_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2335_, 0, v_m_2327_);
lean_ctor_set(v___x_2335_, 1, v___x_2334_);
v___x_2336_ = lean_box(0);
v___x_2337_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2337_, 0, v___x_2335_);
lean_ctor_set(v___x_2337_, 1, v___x_2336_);
v___x_2338_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2338_, 0, v___x_2333_);
lean_ctor_set(v___x_2338_, 1, v___x_2337_);
v___x_2339_ = lean_box(0);
v___x_2340_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2340_, 0, v___x_2338_);
lean_ctor_set(v___x_2340_, 1, v___x_2339_);
v___x_2341_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__0));
v___x_2342_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2342_, 0, v_l_2326_);
lean_ctor_set(v___x_2342_, 1, v___x_2341_);
v___x_2343_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_2343_, 0, v_n_2328_);
v___x_2344_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2344_, 0, v_m_2327_);
lean_ctor_set(v___x_2344_, 1, v___x_2343_);
v___x_2345_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2345_, 0, v___x_2344_);
lean_ctor_set(v___x_2345_, 1, v___x_2336_);
v___x_2346_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2346_, 0, v___x_2342_);
lean_ctor_set(v___x_2346_, 1, v___x_2345_);
v___x_2347_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2347_, 0, v___x_2346_);
lean_ctor_set(v___x_2347_, 1, v___x_2339_);
v___x_2348_ = lp_clauseflow_ClauseFlow_cmpCls(v_a1_2330_, v_m_2327_, v_n_2328_, v_l_2326_, v___x_2341_, v___x_2332_);
v___x_2349_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2349_, 0, v___x_2347_);
lean_ctor_set(v___x_2349_, 1, v___x_2348_);
v___x_2350_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2350_, 0, v___x_2340_);
lean_ctor_set(v___x_2350_, 1, v___x_2349_);
v___x_2351_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2351_, 0, v_facts_2331_);
lean_ctor_set(v___x_2351_, 1, v___x_2350_);
return v___x_2351_;
}
else
{
lean_object* v___x_2352_; 
lean_dec(v_n_2328_);
lean_dec(v_m_2327_);
v___x_2352_ = lp_clauseflow_ClauseFlow_kill(v_l_2326_, v_x_2253_);
lean_dec(v_l_2326_);
return v___x_2352_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_tStmts(lean_object* v_x_2353_, lean_object* v_x_2354_){
_start:
{
if (lean_obj_tag(v_x_2353_) == 0)
{
return v_x_2354_;
}
else
{
lean_object* v_head_2355_; lean_object* v_tail_2356_; lean_object* v___x_2357_; 
v_head_2355_ = lean_ctor_get(v_x_2353_, 0);
lean_inc(v_head_2355_);
v_tail_2356_ = lean_ctor_get(v_x_2353_, 1);
lean_inc(v_tail_2356_);
lean_dec_ref_known(v_x_2353_, 2);
v___x_2357_ = lp_clauseflow_ClauseFlow_tStmt(v_head_2355_, v_x_2354_);
v_x_2353_ = v_tail_2356_;
v_x_2354_ = v___x_2357_;
goto _start;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_optEdge(lean_object* v_other_2359_, lean_object* v_a_2360_){
_start:
{
if (lean_obj_tag(v_other_2359_) == 0)
{
lean_object* v___x_2361_; 
lean_dec_ref(v_a_2360_);
v___x_2361_ = lean_box(0);
return v___x_2361_;
}
else
{
lean_object* v_val_2362_; lean_object* v___x_2363_; lean_object* v___x_2364_; lean_object* v___x_2365_; 
v_val_2362_ = lean_ctor_get(v_other_2359_, 0);
lean_inc(v_val_2362_);
v___x_2363_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2363_, 0, v_val_2362_);
lean_ctor_set(v___x_2363_, 1, v_a_2360_);
v___x_2364_ = lean_box(0);
v___x_2365_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2365_, 0, v___x_2363_);
lean_ctor_set(v___x_2365_, 1, v___x_2364_);
return v___x_2365_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_optEdge___boxed(lean_object* v_other_2366_, lean_object* v_a_2367_){
_start:
{
lean_object* v_res_2368_; 
v_res_2368_ = lp_clauseflow_ClauseFlow_optEdge(v_other_2366_, v_a_2367_);
lean_dec(v_other_2366_);
return v_res_2368_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_knownH_spec__0(lean_object* v_a_2369_, lean_object* v_a_2370_){
_start:
{
if (lean_obj_tag(v_a_2369_) == 0)
{
lean_object* v___x_2371_; 
v___x_2371_ = l_List_reverse___redArg(v_a_2370_);
return v___x_2371_;
}
else
{
lean_object* v_head_2372_; lean_object* v_tail_2373_; lean_object* v___x_2375_; uint8_t v_isShared_2376_; uint8_t v_isSharedCheck_2384_; 
v_head_2372_ = lean_ctor_get(v_a_2369_, 0);
v_tail_2373_ = lean_ctor_get(v_a_2369_, 1);
v_isSharedCheck_2384_ = !lean_is_exclusive(v_a_2369_);
if (v_isSharedCheck_2384_ == 0)
{
v___x_2375_ = v_a_2369_;
v_isShared_2376_ = v_isSharedCheck_2384_;
goto v_resetjp_2374_;
}
else
{
lean_inc(v_tail_2373_);
lean_inc(v_head_2372_);
lean_dec(v_a_2369_);
v___x_2375_ = lean_box(0);
v_isShared_2376_ = v_isSharedCheck_2384_;
goto v_resetjp_2374_;
}
v_resetjp_2374_:
{
lean_object* v_gs_2377_; uint8_t v___x_2378_; 
v_gs_2377_ = lean_ctor_get(v_head_2372_, 0);
v___x_2378_ = l_List_isEmpty___redArg(v_gs_2377_);
if (v___x_2378_ == 0)
{
lean_del_object(v___x_2375_);
lean_dec(v_head_2372_);
v_a_2369_ = v_tail_2373_;
goto _start;
}
else
{
lean_object* v___x_2381_; 
if (v_isShared_2376_ == 0)
{
lean_ctor_set(v___x_2375_, 1, v_a_2370_);
v___x_2381_ = v___x_2375_;
goto v_reusejp_2380_;
}
else
{
lean_object* v_reuseFailAlloc_2383_; 
v_reuseFailAlloc_2383_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2383_, 0, v_head_2372_);
lean_ctor_set(v_reuseFailAlloc_2383_, 1, v_a_2370_);
v___x_2381_ = v_reuseFailAlloc_2383_;
goto v_reusejp_2380_;
}
v_reusejp_2380_:
{
v_a_2369_ = v_tail_2373_;
v_a_2370_ = v___x_2381_;
goto _start;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownH_spec__1(lean_object* v_a_2385_, lean_object* v_a_2386_){
_start:
{
if (lean_obj_tag(v_a_2385_) == 0)
{
lean_object* v___x_2387_; 
v___x_2387_ = lean_array_to_list(v_a_2386_);
return v___x_2387_;
}
else
{
lean_object* v_head_2388_; lean_object* v_tail_2389_; lean_object* v_c_2390_; lean_object* v___x_2391_; lean_object* v___x_2392_; 
v_head_2388_ = lean_ctor_get(v_a_2385_, 0);
v_tail_2389_ = lean_ctor_get(v_a_2385_, 1);
v_c_2390_ = lean_ctor_get(v_head_2388_, 1);
v___x_2391_ = lp_clauseflow_ClauseFlow_Concl_hs(v_c_2390_);
v___x_2392_ = l_List_foldl___at___00Array_appendList_spec__0___redArg(v_a_2386_, v___x_2391_);
v_a_2385_ = v_tail_2389_;
v_a_2386_ = v___x_2392_;
goto _start;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownH_spec__1___boxed(lean_object* v_a_2394_, lean_object* v_a_2395_){
_start:
{
lean_object* v_res_2396_; 
v_res_2396_ = lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownH_spec__1(v_a_2394_, v_a_2395_);
lean_dec(v_a_2394_);
return v_res_2396_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_knownH(lean_object* v_a_2399_){
_start:
{
lean_object* v_cls_2400_; lean_object* v___x_2401_; lean_object* v___x_2402_; lean_object* v___x_2403_; lean_object* v___x_2404_; 
v_cls_2400_ = lean_ctor_get(v_a_2399_, 1);
lean_inc(v_cls_2400_);
lean_dec_ref(v_a_2399_);
v___x_2401_ = lean_box(0);
v___x_2402_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_knownH_spec__0(v_cls_2400_, v___x_2401_);
v___x_2403_ = ((lean_object*)(lp_clauseflow_ClauseFlow_knownH___closed__0));
v___x_2404_ = lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownH_spec__1(v___x_2402_, v___x_2403_);
lean_dec(v___x_2402_);
return v___x_2404_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownP_spec__0(lean_object* v_a_2405_, lean_object* v_a_2406_){
_start:
{
if (lean_obj_tag(v_a_2405_) == 0)
{
lean_object* v___x_2407_; 
v___x_2407_ = lean_array_to_list(v_a_2406_);
return v___x_2407_;
}
else
{
lean_object* v_head_2408_; lean_object* v_tail_2409_; lean_object* v_c_2410_; lean_object* v___x_2411_; lean_object* v___x_2412_; 
v_head_2408_ = lean_ctor_get(v_a_2405_, 0);
v_tail_2409_ = lean_ctor_get(v_a_2405_, 1);
v_c_2410_ = lean_ctor_get(v_head_2408_, 1);
v___x_2411_ = lp_clauseflow_ClauseFlow_Concl_ps(v_c_2410_);
v___x_2412_ = l_List_foldl___at___00Array_appendList_spec__0___redArg(v_a_2406_, v___x_2411_);
v_a_2405_ = v_tail_2409_;
v_a_2406_ = v___x_2412_;
goto _start;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownP_spec__0___boxed(lean_object* v_a_2414_, lean_object* v_a_2415_){
_start:
{
lean_object* v_res_2416_; 
v_res_2416_ = lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownP_spec__0(v_a_2414_, v_a_2415_);
lean_dec(v_a_2414_);
return v_res_2416_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_knownP(lean_object* v_a_2417_){
_start:
{
lean_object* v_cls_2418_; lean_object* v___x_2419_; lean_object* v___x_2420_; lean_object* v___x_2421_; lean_object* v___x_2422_; 
v_cls_2418_ = lean_ctor_get(v_a_2417_, 1);
lean_inc(v_cls_2418_);
lean_dec_ref(v_a_2417_);
v___x_2419_ = lean_box(0);
v___x_2420_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_knownH_spec__0(v_cls_2418_, v___x_2419_);
v___x_2421_ = ((lean_object*)(lp_clauseflow_ClauseFlow_knownH___closed__0));
v___x_2422_ = lp_clauseflow___private_Init_Data_List_Impl_0__List_flatMapTR_go___at___00ClauseFlow_knownP_spec__0(v___x_2420_, v___x_2421_);
lean_dec(v___x_2420_);
return v___x_2422_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_rebase(lean_object* v_H_2423_, lean_object* v_P_2424_, lean_object* v_x_2425_){
_start:
{
if (lean_obj_tag(v_x_2425_) == 0)
{
lean_dec(v_P_2424_);
lean_dec(v_H_2423_);
return v_x_2425_;
}
else
{
lean_object* v_hs_2426_; lean_object* v_ps_2427_; lean_object* v___x_2429_; uint8_t v_isShared_2430_; uint8_t v_isSharedCheck_2438_; 
v_hs_2426_ = lean_ctor_get(v_x_2425_, 0);
v_ps_2427_ = lean_ctor_get(v_x_2425_, 1);
v_isSharedCheck_2438_ = !lean_is_exclusive(v_x_2425_);
if (v_isSharedCheck_2438_ == 0)
{
v___x_2429_ = v_x_2425_;
v_isShared_2430_ = v_isSharedCheck_2438_;
goto v_resetjp_2428_;
}
else
{
lean_inc(v_ps_2427_);
lean_inc(v_hs_2426_);
lean_dec(v_x_2425_);
v___x_2429_ = lean_box(0);
v_isShared_2430_ = v_isSharedCheck_2438_;
goto v_resetjp_2428_;
}
v_resetjp_2428_:
{
lean_object* v___x_2431_; lean_object* v___x_2432_; lean_object* v___x_2433_; lean_object* v___x_2434_; lean_object* v___x_2436_; 
v___x_2431_ = lean_box(0);
lean_inc(v_ps_2427_);
v___x_2432_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_Concl_restrict_spec__0(v_H_2423_, v_ps_2427_, v___x_2431_);
v___x_2433_ = l_List_appendTR___redArg(v_hs_2426_, v___x_2432_);
v___x_2434_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_Concl_restrict_spec__0(v_P_2424_, v_ps_2427_, v___x_2431_);
if (v_isShared_2430_ == 0)
{
lean_ctor_set(v___x_2429_, 1, v___x_2434_);
lean_ctor_set(v___x_2429_, 0, v___x_2433_);
v___x_2436_ = v___x_2429_;
goto v_reusejp_2435_;
}
else
{
lean_object* v_reuseFailAlloc_2437_; 
v_reuseFailAlloc_2437_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2437_, 0, v___x_2433_);
lean_ctor_set(v_reuseFailAlloc_2437_, 1, v___x_2434_);
v___x_2436_ = v_reuseFailAlloc_2437_;
goto v_reusejp_2435_;
}
v_reusejp_2435_:
{
return v___x_2436_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_killAll(lean_object* v_x_2439_, lean_object* v_x_2440_){
_start:
{
if (lean_obj_tag(v_x_2439_) == 0)
{
return v_x_2440_;
}
else
{
lean_object* v_head_2441_; lean_object* v_tail_2442_; lean_object* v___x_2443_; 
v_head_2441_ = lean_ctor_get(v_x_2439_, 0);
v_tail_2442_ = lean_ctor_get(v_x_2439_, 1);
v___x_2443_ = lp_clauseflow_ClauseFlow_kill(v_head_2441_, v_x_2440_);
v_x_2439_ = v_tail_2442_;
v_x_2440_ = v___x_2443_;
goto _start;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_killAll___boxed(lean_object* v_x_2445_, lean_object* v_x_2446_){
_start:
{
lean_object* v_res_2447_; 
v_res_2447_ = lp_clauseflow_ClauseFlow_killAll(v_x_2445_, v_x_2446_);
lean_dec(v_x_2445_);
return v_res_2447_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__0(lean_object* v_l_2448_, lean_object* v_x_2449_, lean_object* v_a_2450_, lean_object* v_a_2451_){
_start:
{
if (lean_obj_tag(v_a_2450_) == 0)
{
lean_object* v___x_2452_; 
lean_dec(v_l_2448_);
v___x_2452_ = l_List_reverse___redArg(v_a_2451_);
return v___x_2452_;
}
else
{
lean_object* v_head_2453_; lean_object* v_tail_2454_; lean_object* v___x_2456_; uint8_t v_isShared_2457_; uint8_t v_isSharedCheck_2477_; 
v_head_2453_ = lean_ctor_get(v_a_2450_, 0);
v_tail_2454_ = lean_ctor_get(v_a_2450_, 1);
v_isSharedCheck_2477_ = !lean_is_exclusive(v_a_2450_);
if (v_isSharedCheck_2477_ == 0)
{
v___x_2456_ = v_a_2450_;
v_isShared_2457_ = v_isSharedCheck_2477_;
goto v_resetjp_2455_;
}
else
{
lean_inc(v_tail_2454_);
lean_inc(v_head_2453_);
lean_dec(v_a_2450_);
v___x_2456_ = lean_box(0);
v_isShared_2457_ = v_isSharedCheck_2477_;
goto v_resetjp_2455_;
}
v_resetjp_2455_:
{
lean_object* v_fst_2458_; lean_object* v_snd_2459_; lean_object* v___x_2461_; uint8_t v_isShared_2462_; uint8_t v_isSharedCheck_2476_; 
v_fst_2458_ = lean_ctor_get(v_head_2453_, 0);
v_snd_2459_ = lean_ctor_get(v_head_2453_, 1);
v_isSharedCheck_2476_ = !lean_is_exclusive(v_head_2453_);
if (v_isSharedCheck_2476_ == 0)
{
v___x_2461_ = v_head_2453_;
v_isShared_2462_ = v_isSharedCheck_2476_;
goto v_resetjp_2460_;
}
else
{
lean_inc(v_snd_2459_);
lean_inc(v_fst_2458_);
lean_dec(v_head_2453_);
v___x_2461_ = lean_box(0);
v_isShared_2462_ = v_isSharedCheck_2476_;
goto v_resetjp_2460_;
}
v_resetjp_2460_:
{
lean_object* v_facts_2463_; lean_object* v_cls_2464_; lean_object* v___x_2465_; lean_object* v___x_2467_; 
v_facts_2463_ = lean_ctor_get(v_x_2449_, 0);
v_cls_2464_ = lean_ctor_get(v_x_2449_, 1);
v___x_2465_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v___x_2465_, 0, v_fst_2458_);
lean_inc(v_l_2448_);
if (v_isShared_2462_ == 0)
{
lean_ctor_set(v___x_2461_, 1, v___x_2465_);
lean_ctor_set(v___x_2461_, 0, v_l_2448_);
v___x_2467_ = v___x_2461_;
goto v_reusejp_2466_;
}
else
{
lean_object* v_reuseFailAlloc_2475_; 
v_reuseFailAlloc_2475_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2475_, 0, v_l_2448_);
lean_ctor_set(v_reuseFailAlloc_2475_, 1, v___x_2465_);
v___x_2467_ = v_reuseFailAlloc_2475_;
goto v_reusejp_2466_;
}
v_reusejp_2466_:
{
lean_object* v___x_2469_; 
lean_inc(v_facts_2463_);
if (v_isShared_2457_ == 0)
{
lean_ctor_set(v___x_2456_, 1, v_facts_2463_);
lean_ctor_set(v___x_2456_, 0, v___x_2467_);
v___x_2469_ = v___x_2456_;
goto v_reusejp_2468_;
}
else
{
lean_object* v_reuseFailAlloc_2474_; 
v_reuseFailAlloc_2474_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2474_, 0, v___x_2467_);
lean_ctor_set(v_reuseFailAlloc_2474_, 1, v_facts_2463_);
v___x_2469_ = v_reuseFailAlloc_2474_;
goto v_reusejp_2468_;
}
v_reusejp_2468_:
{
lean_object* v___x_2470_; lean_object* v___x_2471_; lean_object* v___x_2472_; 
lean_inc(v_cls_2464_);
v___x_2470_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2470_, 0, v___x_2469_);
lean_ctor_set(v___x_2470_, 1, v_cls_2464_);
v___x_2471_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2471_, 0, v_snd_2459_);
lean_ctor_set(v___x_2471_, 1, v___x_2470_);
v___x_2472_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2472_, 0, v___x_2471_);
lean_ctor_set(v___x_2472_, 1, v_a_2451_);
v_a_2450_ = v_tail_2454_;
v_a_2451_ = v___x_2472_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__0___boxed(lean_object* v_l_2478_, lean_object* v_x_2479_, lean_object* v_a_2480_, lean_object* v_a_2481_){
_start:
{
lean_object* v_res_2482_; 
v_res_2482_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__0(v_l_2478_, v_x_2479_, v_a_2480_, v_a_2481_);
lean_dec_ref(v_x_2479_);
return v_res_2482_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__1(lean_object* v_l_2483_, lean_object* v_a_2484_, lean_object* v_a_2485_){
_start:
{
if (lean_obj_tag(v_a_2484_) == 0)
{
lean_object* v___x_2486_; 
lean_dec(v_l_2483_);
v___x_2486_ = l_List_reverse___redArg(v_a_2485_);
return v___x_2486_;
}
else
{
lean_object* v_head_2487_; lean_object* v_tail_2488_; lean_object* v___x_2490_; uint8_t v_isShared_2491_; uint8_t v_isSharedCheck_2506_; 
v_head_2487_ = lean_ctor_get(v_a_2484_, 0);
v_tail_2488_ = lean_ctor_get(v_a_2484_, 1);
v_isSharedCheck_2506_ = !lean_is_exclusive(v_a_2484_);
if (v_isSharedCheck_2506_ == 0)
{
v___x_2490_ = v_a_2484_;
v_isShared_2491_ = v_isSharedCheck_2506_;
goto v_resetjp_2489_;
}
else
{
lean_inc(v_tail_2488_);
lean_inc(v_head_2487_);
lean_dec(v_a_2484_);
v___x_2490_ = lean_box(0);
v_isShared_2491_ = v_isSharedCheck_2506_;
goto v_resetjp_2489_;
}
v_resetjp_2489_:
{
lean_object* v_fst_2492_; lean_object* v___x_2494_; uint8_t v_isShared_2495_; uint8_t v_isSharedCheck_2504_; 
v_fst_2492_ = lean_ctor_get(v_head_2487_, 0);
v_isSharedCheck_2504_ = !lean_is_exclusive(v_head_2487_);
if (v_isSharedCheck_2504_ == 0)
{
lean_object* v_unused_2505_; 
v_unused_2505_ = lean_ctor_get(v_head_2487_, 1);
lean_dec(v_unused_2505_);
v___x_2494_ = v_head_2487_;
v_isShared_2495_ = v_isSharedCheck_2504_;
goto v_resetjp_2493_;
}
else
{
lean_inc(v_fst_2492_);
lean_dec(v_head_2487_);
v___x_2494_ = lean_box(0);
v_isShared_2495_ = v_isSharedCheck_2504_;
goto v_resetjp_2493_;
}
v_resetjp_2493_:
{
lean_object* v___x_2496_; lean_object* v___x_2498_; 
v___x_2496_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_2496_, 0, v_fst_2492_);
lean_inc(v_l_2483_);
if (v_isShared_2495_ == 0)
{
lean_ctor_set(v___x_2494_, 1, v___x_2496_);
lean_ctor_set(v___x_2494_, 0, v_l_2483_);
v___x_2498_ = v___x_2494_;
goto v_reusejp_2497_;
}
else
{
lean_object* v_reuseFailAlloc_2503_; 
v_reuseFailAlloc_2503_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2503_, 0, v_l_2483_);
lean_ctor_set(v_reuseFailAlloc_2503_, 1, v___x_2496_);
v___x_2498_ = v_reuseFailAlloc_2503_;
goto v_reusejp_2497_;
}
v_reusejp_2497_:
{
lean_object* v___x_2500_; 
if (v_isShared_2491_ == 0)
{
lean_ctor_set(v___x_2490_, 1, v_a_2485_);
lean_ctor_set(v___x_2490_, 0, v___x_2498_);
v___x_2500_ = v___x_2490_;
goto v_reusejp_2499_;
}
else
{
lean_object* v_reuseFailAlloc_2502_; 
v_reuseFailAlloc_2502_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2502_, 0, v___x_2498_);
lean_ctor_set(v_reuseFailAlloc_2502_, 1, v_a_2485_);
v___x_2500_ = v_reuseFailAlloc_2502_;
goto v_reusejp_2499_;
}
v_reusejp_2499_:
{
v_a_2484_ = v_tail_2488_;
v_a_2485_ = v___x_2500_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__5(lean_object* v_src_2507_, lean_object* v_ps_2508_, lean_object* v_a_2509_, lean_object* v_a_2510_){
_start:
{
if (lean_obj_tag(v_a_2509_) == 0)
{
lean_object* v___x_2511_; 
v___x_2511_ = l_List_reverse___redArg(v_a_2510_);
return v___x_2511_;
}
else
{
lean_object* v_head_2512_; lean_object* v_tail_2513_; lean_object* v___x_2515_; uint8_t v_isShared_2516_; uint8_t v_isSharedCheck_2526_; 
v_head_2512_ = lean_ctor_get(v_a_2509_, 0);
v_tail_2513_ = lean_ctor_get(v_a_2509_, 1);
v_isSharedCheck_2526_ = !lean_is_exclusive(v_a_2509_);
if (v_isSharedCheck_2526_ == 0)
{
v___x_2515_ = v_a_2509_;
v_isShared_2516_ = v_isSharedCheck_2526_;
goto v_resetjp_2514_;
}
else
{
lean_inc(v_tail_2513_);
lean_inc(v_head_2512_);
lean_dec(v_a_2509_);
v___x_2515_ = lean_box(0);
v_isShared_2516_ = v_isSharedCheck_2526_;
goto v_resetjp_2514_;
}
v_resetjp_2514_:
{
uint8_t v___y_2518_; uint8_t v___x_2524_; 
v___x_2524_ = lp_clauseflow_ClauseFlow_mentions(v_src_2507_, v_head_2512_);
if (v___x_2524_ == 0)
{
uint8_t v___x_2525_; 
v___x_2525_ = lp_clauseflow_ClauseFlow_mentions(v_ps_2508_, v_head_2512_);
v___y_2518_ = v___x_2525_;
goto v___jp_2517_;
}
else
{
v___y_2518_ = v___x_2524_;
goto v___jp_2517_;
}
v___jp_2517_:
{
if (v___y_2518_ == 0)
{
lean_del_object(v___x_2515_);
lean_dec(v_head_2512_);
v_a_2509_ = v_tail_2513_;
goto _start;
}
else
{
lean_object* v___x_2521_; 
if (v_isShared_2516_ == 0)
{
lean_ctor_set(v___x_2515_, 1, v_a_2510_);
v___x_2521_ = v___x_2515_;
goto v_reusejp_2520_;
}
else
{
lean_object* v_reuseFailAlloc_2523_; 
v_reuseFailAlloc_2523_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2523_, 0, v_head_2512_);
lean_ctor_set(v_reuseFailAlloc_2523_, 1, v_a_2510_);
v___x_2521_ = v_reuseFailAlloc_2523_;
goto v_reusejp_2520_;
}
v_reusejp_2520_:
{
v_a_2509_ = v_tail_2513_;
v_a_2510_ = v___x_2521_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__5___boxed(lean_object* v_src_2527_, lean_object* v_ps_2528_, lean_object* v_a_2529_, lean_object* v_a_2530_){
_start:
{
lean_object* v_res_2531_; 
v_res_2531_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__5(v_src_2527_, v_ps_2528_, v_a_2529_, v_a_2530_);
lean_dec(v_ps_2528_);
lean_dec(v_src_2527_);
return v_res_2531_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__2(lean_object* v_x_2532_, lean_object* v_a_2533_, lean_object* v_a_2534_){
_start:
{
if (lean_obj_tag(v_a_2533_) == 0)
{
lean_object* v___x_2535_; 
lean_dec_ref(v_x_2532_);
v___x_2535_ = l_List_reverse___redArg(v_a_2534_);
return v___x_2535_;
}
else
{
lean_object* v_head_2536_; lean_object* v_tail_2537_; lean_object* v___x_2539_; uint8_t v_isShared_2540_; uint8_t v_isSharedCheck_2557_; 
v_head_2536_ = lean_ctor_get(v_a_2533_, 0);
v_tail_2537_ = lean_ctor_get(v_a_2533_, 1);
v_isSharedCheck_2557_ = !lean_is_exclusive(v_a_2533_);
if (v_isSharedCheck_2557_ == 0)
{
v___x_2539_ = v_a_2533_;
v_isShared_2540_ = v_isSharedCheck_2557_;
goto v_resetjp_2538_;
}
else
{
lean_inc(v_tail_2537_);
lean_inc(v_head_2536_);
lean_dec(v_a_2533_);
v___x_2539_ = lean_box(0);
v_isShared_2540_ = v_isSharedCheck_2557_;
goto v_resetjp_2538_;
}
v_resetjp_2538_:
{
lean_object* v_gs_2541_; lean_object* v_c_2542_; lean_object* v___x_2544_; uint8_t v_isShared_2545_; uint8_t v_isSharedCheck_2556_; 
v_gs_2541_ = lean_ctor_get(v_head_2536_, 0);
v_c_2542_ = lean_ctor_get(v_head_2536_, 1);
v_isSharedCheck_2556_ = !lean_is_exclusive(v_head_2536_);
if (v_isSharedCheck_2556_ == 0)
{
v___x_2544_ = v_head_2536_;
v_isShared_2545_ = v_isSharedCheck_2556_;
goto v_resetjp_2543_;
}
else
{
lean_inc(v_c_2542_);
lean_inc(v_gs_2541_);
lean_dec(v_head_2536_);
v___x_2544_ = lean_box(0);
v_isShared_2545_ = v_isSharedCheck_2556_;
goto v_resetjp_2543_;
}
v_resetjp_2543_:
{
lean_object* v___x_2546_; lean_object* v___x_2547_; lean_object* v___x_2548_; lean_object* v___x_2550_; 
lean_inc_ref_n(v_x_2532_, 2);
v___x_2546_ = lp_clauseflow_ClauseFlow_knownH(v_x_2532_);
v___x_2547_ = lp_clauseflow_ClauseFlow_knownP(v_x_2532_);
v___x_2548_ = lp_clauseflow_ClauseFlow_rebase(v___x_2546_, v___x_2547_, v_c_2542_);
if (v_isShared_2545_ == 0)
{
lean_ctor_set(v___x_2544_, 1, v___x_2548_);
v___x_2550_ = v___x_2544_;
goto v_reusejp_2549_;
}
else
{
lean_object* v_reuseFailAlloc_2555_; 
v_reuseFailAlloc_2555_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2555_, 0, v_gs_2541_);
lean_ctor_set(v_reuseFailAlloc_2555_, 1, v___x_2548_);
v___x_2550_ = v_reuseFailAlloc_2555_;
goto v_reusejp_2549_;
}
v_reusejp_2549_:
{
lean_object* v___x_2552_; 
if (v_isShared_2540_ == 0)
{
lean_ctor_set(v___x_2539_, 1, v_a_2534_);
lean_ctor_set(v___x_2539_, 0, v___x_2550_);
v___x_2552_ = v___x_2539_;
goto v_reusejp_2551_;
}
else
{
lean_object* v_reuseFailAlloc_2554_; 
v_reuseFailAlloc_2554_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2554_, 0, v___x_2550_);
lean_ctor_set(v_reuseFailAlloc_2554_, 1, v_a_2534_);
v___x_2552_ = v_reuseFailAlloc_2554_;
goto v_reusejp_2551_;
}
v_reusejp_2551_:
{
v_a_2533_ = v_tail_2537_;
v_a_2534_ = v___x_2552_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__3(lean_object* v_src_2558_, lean_object* v_a_2559_, lean_object* v_a_2560_){
_start:
{
if (lean_obj_tag(v_a_2559_) == 0)
{
lean_object* v___x_2561_; 
lean_dec(v_src_2558_);
v___x_2561_ = l_List_reverse___redArg(v_a_2560_);
return v___x_2561_;
}
else
{
lean_object* v_head_2562_; lean_object* v_tail_2563_; lean_object* v___x_2565_; uint8_t v_isShared_2566_; uint8_t v_isSharedCheck_2577_; 
v_head_2562_ = lean_ctor_get(v_a_2559_, 0);
v_tail_2563_ = lean_ctor_get(v_a_2559_, 1);
v_isSharedCheck_2577_ = !lean_is_exclusive(v_a_2559_);
if (v_isSharedCheck_2577_ == 0)
{
v___x_2565_ = v_a_2559_;
v_isShared_2566_ = v_isSharedCheck_2577_;
goto v_resetjp_2564_;
}
else
{
lean_inc(v_tail_2563_);
lean_inc(v_head_2562_);
lean_dec(v_a_2559_);
v___x_2565_ = lean_box(0);
v_isShared_2566_ = v_isSharedCheck_2577_;
goto v_resetjp_2564_;
}
v_resetjp_2564_:
{
lean_object* v___x_2567_; lean_object* v___x_2568_; lean_object* v___x_2569_; lean_object* v___x_2570_; uint8_t v___x_2571_; 
v___x_2567_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__0));
v___x_2568_ = lean_alloc_closure((void*)(l_instDecidableEqNat___boxed), 2, 0);
v___x_2569_ = lean_alloc_closure((void*)(lp_clauseflow_ClauseFlow_instDecidableEqTest___boxed), 2, 0);
lean_inc(v_src_2558_);
v___x_2570_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2570_, 0, v_src_2558_);
lean_ctor_set(v___x_2570_, 1, v___x_2567_);
lean_inc(v_head_2562_);
v___x_2571_ = l_instDecidableEqProd___redArg(v___x_2568_, v___x_2569_, v_head_2562_, v___x_2570_);
if (v___x_2571_ == 0)
{
lean_object* v___x_2573_; 
if (v_isShared_2566_ == 0)
{
lean_ctor_set(v___x_2565_, 1, v_a_2560_);
v___x_2573_ = v___x_2565_;
goto v_reusejp_2572_;
}
else
{
lean_object* v_reuseFailAlloc_2575_; 
v_reuseFailAlloc_2575_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2575_, 0, v_head_2562_);
lean_ctor_set(v_reuseFailAlloc_2575_, 1, v_a_2560_);
v___x_2573_ = v_reuseFailAlloc_2575_;
goto v_reusejp_2572_;
}
v_reusejp_2572_:
{
v_a_2559_ = v_tail_2563_;
v_a_2560_ = v___x_2573_;
goto _start;
}
}
else
{
lean_del_object(v___x_2565_);
lean_dec(v_head_2562_);
v_a_2559_ = v_tail_2563_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6_spec__6(lean_object* v_dst_2578_, lean_object* v_src_2579_, lean_object* v_ps_2580_, lean_object* v_pd_2581_, lean_object* v_a_2582_, lean_object* v_a_2583_){
_start:
{
if (lean_obj_tag(v_a_2582_) == 0)
{
lean_object* v___x_2584_; 
lean_dec(v_pd_2581_);
lean_dec(v_src_2579_);
lean_dec(v_dst_2578_);
v___x_2584_ = l_List_reverse___redArg(v_a_2583_);
return v___x_2584_;
}
else
{
lean_object* v_head_2585_; lean_object* v_tail_2586_; lean_object* v___x_2588_; uint8_t v_isShared_2589_; uint8_t v_isSharedCheck_2609_; 
v_head_2585_ = lean_ctor_get(v_a_2582_, 0);
v_tail_2586_ = lean_ctor_get(v_a_2582_, 1);
v_isSharedCheck_2609_ = !lean_is_exclusive(v_a_2582_);
if (v_isSharedCheck_2609_ == 0)
{
v___x_2588_ = v_a_2582_;
v_isShared_2589_ = v_isSharedCheck_2609_;
goto v_resetjp_2587_;
}
else
{
lean_inc(v_tail_2586_);
lean_inc(v_head_2585_);
lean_dec(v_a_2582_);
v___x_2588_ = lean_box(0);
v_isShared_2589_ = v_isSharedCheck_2609_;
goto v_resetjp_2587_;
}
v_resetjp_2587_:
{
lean_object* v_gs_2590_; lean_object* v_c_2591_; lean_object* v___x_2593_; uint8_t v_isShared_2594_; uint8_t v_isSharedCheck_2608_; 
v_gs_2590_ = lean_ctor_get(v_head_2585_, 0);
v_c_2591_ = lean_ctor_get(v_head_2585_, 1);
v_isSharedCheck_2608_ = !lean_is_exclusive(v_head_2585_);
if (v_isSharedCheck_2608_ == 0)
{
v___x_2593_ = v_head_2585_;
v_isShared_2594_ = v_isSharedCheck_2608_;
goto v_resetjp_2592_;
}
else
{
lean_inc(v_c_2591_);
lean_inc(v_gs_2590_);
lean_dec(v_head_2585_);
v___x_2593_ = lean_box(0);
v_isShared_2594_ = v_isSharedCheck_2608_;
goto v_resetjp_2592_;
}
v_resetjp_2592_:
{
lean_object* v___x_2595_; lean_object* v___x_2596_; lean_object* v___x_2597_; lean_object* v___x_2598_; lean_object* v___x_2599_; lean_object* v___x_2601_; 
v___x_2595_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__0));
lean_inc(v_dst_2578_);
v___x_2596_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2596_, 0, v_dst_2578_);
lean_ctor_set(v___x_2596_, 1, v___x_2595_);
v___x_2597_ = lean_box(0);
lean_inc(v_src_2579_);
v___x_2598_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__3(v_src_2579_, v_gs_2590_, v___x_2597_);
lean_inc(v_pd_2581_);
v___x_2599_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0(v_ps_2580_, v_pd_2581_, v___x_2598_, v___x_2597_);
if (v_isShared_2589_ == 0)
{
lean_ctor_set(v___x_2588_, 1, v___x_2599_);
lean_ctor_set(v___x_2588_, 0, v___x_2596_);
v___x_2601_ = v___x_2588_;
goto v_reusejp_2600_;
}
else
{
lean_object* v_reuseFailAlloc_2607_; 
v_reuseFailAlloc_2607_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2607_, 0, v___x_2596_);
lean_ctor_set(v_reuseFailAlloc_2607_, 1, v___x_2599_);
v___x_2601_ = v_reuseFailAlloc_2607_;
goto v_reusejp_2600_;
}
v_reusejp_2600_:
{
lean_object* v___x_2603_; 
if (v_isShared_2594_ == 0)
{
lean_ctor_set(v___x_2593_, 0, v___x_2601_);
v___x_2603_ = v___x_2593_;
goto v_reusejp_2602_;
}
else
{
lean_object* v_reuseFailAlloc_2606_; 
v_reuseFailAlloc_2606_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2606_, 0, v___x_2601_);
lean_ctor_set(v_reuseFailAlloc_2606_, 1, v_c_2591_);
v___x_2603_ = v_reuseFailAlloc_2606_;
goto v_reusejp_2602_;
}
v_reusejp_2602_:
{
lean_object* v___x_2604_; 
v___x_2604_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2604_, 0, v___x_2603_);
lean_ctor_set(v___x_2604_, 1, v_a_2583_);
v_a_2582_ = v_tail_2586_;
v_a_2583_ = v___x_2604_;
goto _start;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6_spec__6___boxed(lean_object* v_dst_2610_, lean_object* v_src_2611_, lean_object* v_ps_2612_, lean_object* v_pd_2613_, lean_object* v_a_2614_, lean_object* v_a_2615_){
_start:
{
lean_object* v_res_2616_; 
v_res_2616_ = lp_clauseflow_List_mapTR_loop___at___00List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6_spec__6(v_dst_2610_, v_src_2611_, v_ps_2612_, v_pd_2613_, v_a_2614_, v_a_2615_);
lean_dec(v_ps_2612_);
return v_res_2616_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6(lean_object* v_src_2617_, lean_object* v_dst_2618_, lean_object* v_ps_2619_, lean_object* v_pd_2620_, lean_object* v_a_2621_, lean_object* v_a_2622_){
_start:
{
if (lean_obj_tag(v_a_2621_) == 0)
{
lean_object* v___x_2623_; 
lean_dec(v_pd_2620_);
lean_dec(v_dst_2618_);
lean_dec(v_src_2617_);
v___x_2623_ = l_List_reverse___redArg(v_a_2622_);
return v___x_2623_;
}
else
{
lean_object* v_head_2624_; lean_object* v_tail_2625_; lean_object* v___x_2627_; uint8_t v_isShared_2628_; uint8_t v_isSharedCheck_2648_; 
v_head_2624_ = lean_ctor_get(v_a_2621_, 0);
v_tail_2625_ = lean_ctor_get(v_a_2621_, 1);
v_isSharedCheck_2648_ = !lean_is_exclusive(v_a_2621_);
if (v_isSharedCheck_2648_ == 0)
{
v___x_2627_ = v_a_2621_;
v_isShared_2628_ = v_isSharedCheck_2648_;
goto v_resetjp_2626_;
}
else
{
lean_inc(v_tail_2625_);
lean_inc(v_head_2624_);
lean_dec(v_a_2621_);
v___x_2627_ = lean_box(0);
v_isShared_2628_ = v_isSharedCheck_2648_;
goto v_resetjp_2626_;
}
v_resetjp_2626_:
{
lean_object* v_gs_2629_; lean_object* v_c_2630_; lean_object* v___x_2632_; uint8_t v_isShared_2633_; uint8_t v_isSharedCheck_2647_; 
v_gs_2629_ = lean_ctor_get(v_head_2624_, 0);
v_c_2630_ = lean_ctor_get(v_head_2624_, 1);
v_isSharedCheck_2647_ = !lean_is_exclusive(v_head_2624_);
if (v_isSharedCheck_2647_ == 0)
{
v___x_2632_ = v_head_2624_;
v_isShared_2633_ = v_isSharedCheck_2647_;
goto v_resetjp_2631_;
}
else
{
lean_inc(v_c_2630_);
lean_inc(v_gs_2629_);
lean_dec(v_head_2624_);
v___x_2632_ = lean_box(0);
v_isShared_2633_ = v_isSharedCheck_2647_;
goto v_resetjp_2631_;
}
v_resetjp_2631_:
{
lean_object* v___x_2634_; lean_object* v___x_2635_; lean_object* v___x_2636_; lean_object* v___x_2637_; lean_object* v___x_2638_; lean_object* v___x_2640_; 
v___x_2634_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__0));
lean_inc(v_dst_2618_);
v___x_2635_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2635_, 0, v_dst_2618_);
lean_ctor_set(v___x_2635_, 1, v___x_2634_);
v___x_2636_ = lean_box(0);
lean_inc(v_src_2617_);
v___x_2637_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__3(v_src_2617_, v_gs_2629_, v___x_2636_);
lean_inc(v_pd_2620_);
v___x_2638_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_renameCl_spec__0(v_ps_2619_, v_pd_2620_, v___x_2637_, v___x_2636_);
if (v_isShared_2628_ == 0)
{
lean_ctor_set(v___x_2627_, 1, v___x_2638_);
lean_ctor_set(v___x_2627_, 0, v___x_2635_);
v___x_2640_ = v___x_2627_;
goto v_reusejp_2639_;
}
else
{
lean_object* v_reuseFailAlloc_2646_; 
v_reuseFailAlloc_2646_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2646_, 0, v___x_2635_);
lean_ctor_set(v_reuseFailAlloc_2646_, 1, v___x_2638_);
v___x_2640_ = v_reuseFailAlloc_2646_;
goto v_reusejp_2639_;
}
v_reusejp_2639_:
{
lean_object* v___x_2642_; 
if (v_isShared_2633_ == 0)
{
lean_ctor_set(v___x_2632_, 0, v___x_2640_);
v___x_2642_ = v___x_2632_;
goto v_reusejp_2641_;
}
else
{
lean_object* v_reuseFailAlloc_2645_; 
v_reuseFailAlloc_2645_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2645_, 0, v___x_2640_);
lean_ctor_set(v_reuseFailAlloc_2645_, 1, v_c_2630_);
v___x_2642_ = v_reuseFailAlloc_2645_;
goto v_reusejp_2641_;
}
v_reusejp_2641_:
{
lean_object* v___x_2643_; lean_object* v___x_2644_; 
v___x_2643_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2643_, 0, v___x_2642_);
lean_ctor_set(v___x_2643_, 1, v_a_2622_);
v___x_2644_ = lp_clauseflow_List_mapTR_loop___at___00List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6_spec__6(v_dst_2618_, v_src_2617_, v_ps_2619_, v_pd_2620_, v_tail_2625_, v___x_2643_);
return v___x_2644_;
}
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6___boxed(lean_object* v_src_2649_, lean_object* v_dst_2650_, lean_object* v_ps_2651_, lean_object* v_pd_2652_, lean_object* v_a_2653_, lean_object* v_a_2654_){
_start:
{
lean_object* v_res_2655_; 
v_res_2655_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6(v_src_2649_, v_dst_2650_, v_ps_2651_, v_pd_2652_, v_a_2653_, v_a_2654_);
lean_dec(v_ps_2651_);
return v_res_2655_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__4(lean_object* v_a_2656_, lean_object* v_a_2657_){
_start:
{
if (lean_obj_tag(v_a_2656_) == 0)
{
lean_object* v___x_2658_; 
v___x_2658_ = l_List_reverse___redArg(v_a_2657_);
return v___x_2658_;
}
else
{
lean_object* v_head_2659_; lean_object* v_tail_2660_; lean_object* v___x_2662_; uint8_t v_isShared_2663_; uint8_t v_isSharedCheck_2669_; 
v_head_2659_ = lean_ctor_get(v_a_2656_, 0);
v_tail_2660_ = lean_ctor_get(v_a_2656_, 1);
v_isSharedCheck_2669_ = !lean_is_exclusive(v_a_2656_);
if (v_isSharedCheck_2669_ == 0)
{
v___x_2662_ = v_a_2656_;
v_isShared_2663_ = v_isSharedCheck_2669_;
goto v_resetjp_2661_;
}
else
{
lean_inc(v_tail_2660_);
lean_inc(v_head_2659_);
lean_dec(v_a_2656_);
v___x_2662_ = lean_box(0);
v_isShared_2663_ = v_isSharedCheck_2669_;
goto v_resetjp_2661_;
}
v_resetjp_2661_:
{
lean_object* v___x_2664_; lean_object* v___x_2666_; 
v___x_2664_ = lp_clauseflow_ClauseFlow_factCl(v_head_2659_);
if (v_isShared_2663_ == 0)
{
lean_ctor_set(v___x_2662_, 1, v_a_2657_);
lean_ctor_set(v___x_2662_, 0, v___x_2664_);
v___x_2666_ = v___x_2662_;
goto v_reusejp_2665_;
}
else
{
lean_object* v_reuseFailAlloc_2668_; 
v_reuseFailAlloc_2668_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2668_, 0, v___x_2664_);
lean_ctor_set(v_reuseFailAlloc_2668_, 1, v_a_2657_);
v___x_2666_ = v_reuseFailAlloc_2668_;
goto v_reusejp_2665_;
}
v_reusejp_2665_:
{
v_a_2656_ = v_tail_2660_;
v_a_2657_ = v___x_2666_;
goto _start;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_tTerm(lean_object* v_x_2672_, lean_object* v_x_2673_){
_start:
{
switch(lean_obj_tag(v_x_2672_))
{
case 0:
{
lean_object* v_b_2674_; lean_object* v___x_2675_; lean_object* v___x_2676_; lean_object* v___x_2677_; 
v_b_2674_ = lean_ctor_get(v_x_2672_, 0);
lean_inc(v_b_2674_);
lean_dec_ref_known(v_x_2672_, 1);
v___x_2675_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2675_, 0, v_b_2674_);
lean_ctor_set(v___x_2675_, 1, v_x_2673_);
v___x_2676_ = lean_box(0);
v___x_2677_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2677_, 0, v___x_2675_);
lean_ctor_set(v___x_2677_, 1, v___x_2676_);
return v___x_2677_;
}
case 1:
{
lean_object* v_l_2678_; lean_object* v_cases_2679_; lean_object* v_other_2680_; lean_object* v_facts_2681_; lean_object* v_cls_2682_; lean_object* v___x_2683_; lean_object* v___x_2684_; lean_object* v___x_2686_; uint8_t v_isShared_2687_; uint8_t v_isSharedCheck_2695_; 
v_l_2678_ = lean_ctor_get(v_x_2672_, 0);
lean_inc_n(v_l_2678_, 2);
v_cases_2679_ = lean_ctor_get(v_x_2672_, 1);
lean_inc_n(v_cases_2679_, 2);
v_other_2680_ = lean_ctor_get(v_x_2672_, 2);
lean_inc(v_other_2680_);
lean_dec_ref_known(v_x_2672_, 3);
v_facts_2681_ = lean_ctor_get(v_x_2673_, 0);
lean_inc(v_facts_2681_);
v_cls_2682_ = lean_ctor_get(v_x_2673_, 1);
lean_inc(v_cls_2682_);
v___x_2683_ = lean_box(0);
v___x_2684_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__0(v_l_2678_, v_x_2673_, v_cases_2679_, v___x_2683_);
v_isSharedCheck_2695_ = !lean_is_exclusive(v_x_2673_);
if (v_isSharedCheck_2695_ == 0)
{
lean_object* v_unused_2696_; lean_object* v_unused_2697_; 
v_unused_2696_ = lean_ctor_get(v_x_2673_, 1);
lean_dec(v_unused_2696_);
v_unused_2697_ = lean_ctor_get(v_x_2673_, 0);
lean_dec(v_unused_2697_);
v___x_2686_ = v_x_2673_;
v_isShared_2687_ = v_isSharedCheck_2695_;
goto v_resetjp_2685_;
}
else
{
lean_dec(v_x_2673_);
v___x_2686_ = lean_box(0);
v_isShared_2687_ = v_isSharedCheck_2695_;
goto v_resetjp_2685_;
}
v_resetjp_2685_:
{
lean_object* v___x_2688_; lean_object* v___x_2689_; lean_object* v___x_2691_; 
v___x_2688_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__1(v_l_2678_, v_cases_2679_, v___x_2683_);
v___x_2689_ = l_List_appendTR___redArg(v___x_2688_, v_facts_2681_);
if (v_isShared_2687_ == 0)
{
lean_ctor_set(v___x_2686_, 0, v___x_2689_);
v___x_2691_ = v___x_2686_;
goto v_reusejp_2690_;
}
else
{
lean_object* v_reuseFailAlloc_2694_; 
v_reuseFailAlloc_2694_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2694_, 0, v___x_2689_);
lean_ctor_set(v_reuseFailAlloc_2694_, 1, v_cls_2682_);
v___x_2691_ = v_reuseFailAlloc_2694_;
goto v_reusejp_2690_;
}
v_reusejp_2690_:
{
lean_object* v___x_2692_; lean_object* v___x_2693_; 
v___x_2692_ = lp_clauseflow_ClauseFlow_optEdge(v_other_2680_, v___x_2691_);
lean_dec(v_other_2680_);
v___x_2693_ = l_List_appendTR___redArg(v___x_2684_, v___x_2692_);
return v___x_2693_;
}
}
}
case 2:
{
lean_object* v_dsts_2698_; lean_object* v_b_2699_; lean_object* v_same_2700_; lean_object* v_post_2701_; lean_object* v___x_2702_; lean_object* v_a1_2703_; lean_object* v_facts_2704_; lean_object* v_cls_2705_; lean_object* v___x_2707_; uint8_t v_isShared_2708_; uint8_t v_isSharedCheck_2717_; 
v_dsts_2698_ = lean_ctor_get(v_x_2672_, 0);
lean_inc(v_dsts_2698_);
v_b_2699_ = lean_ctor_get(v_x_2672_, 1);
lean_inc(v_b_2699_);
v_same_2700_ = lean_ctor_get(v_x_2672_, 2);
lean_inc(v_same_2700_);
v_post_2701_ = lean_ctor_get(v_x_2672_, 3);
lean_inc(v_post_2701_);
lean_dec_ref_known(v_x_2672_, 4);
lean_inc_ref(v_x_2673_);
v___x_2702_ = lp_clauseflow_ClauseFlow_clobber(v_same_2700_, v_x_2673_);
v_a1_2703_ = lp_clauseflow_ClauseFlow_killAll(v_dsts_2698_, v___x_2702_);
lean_dec(v_dsts_2698_);
v_facts_2704_ = lean_ctor_get(v_a1_2703_, 0);
v_cls_2705_ = lean_ctor_get(v_a1_2703_, 1);
v_isSharedCheck_2717_ = !lean_is_exclusive(v_a1_2703_);
if (v_isSharedCheck_2717_ == 0)
{
v___x_2707_ = v_a1_2703_;
v_isShared_2708_ = v_isSharedCheck_2717_;
goto v_resetjp_2706_;
}
else
{
lean_inc(v_cls_2705_);
lean_inc(v_facts_2704_);
lean_dec(v_a1_2703_);
v___x_2707_ = lean_box(0);
v_isShared_2708_ = v_isSharedCheck_2717_;
goto v_resetjp_2706_;
}
v_resetjp_2706_:
{
lean_object* v___x_2709_; lean_object* v___x_2710_; lean_object* v___x_2711_; lean_object* v___x_2713_; 
v___x_2709_ = lean_box(0);
v___x_2710_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__2(v_x_2673_, v_post_2701_, v___x_2709_);
v___x_2711_ = l_List_appendTR___redArg(v___x_2710_, v_cls_2705_);
if (v_isShared_2708_ == 0)
{
lean_ctor_set(v___x_2707_, 1, v___x_2711_);
v___x_2713_ = v___x_2707_;
goto v_reusejp_2712_;
}
else
{
lean_object* v_reuseFailAlloc_2716_; 
v_reuseFailAlloc_2716_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2716_, 0, v_facts_2704_);
lean_ctor_set(v_reuseFailAlloc_2716_, 1, v___x_2711_);
v___x_2713_ = v_reuseFailAlloc_2716_;
goto v_reusejp_2712_;
}
v_reusejp_2712_:
{
lean_object* v___x_2714_; lean_object* v___x_2715_; 
v___x_2714_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2714_, 0, v_b_2699_);
lean_ctor_set(v___x_2714_, 1, v___x_2713_);
v___x_2715_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2715_, 0, v___x_2714_);
lean_ctor_set(v___x_2715_, 1, v___x_2709_);
return v___x_2715_;
}
}
}
case 3:
{
lean_object* v_dst_2718_; lean_object* v_src_2719_; lean_object* v_pd_2720_; lean_object* v_ps_2721_; lean_object* v_b_2722_; lean_object* v___x_2723_; lean_object* v_a1_2724_; uint8_t v___x_2729_; uint8_t v___x_2730_; uint8_t v___x_2731_; uint8_t v___y_2733_; uint8_t v___x_2772_; 
v_dst_2718_ = lean_ctor_get(v_x_2672_, 0);
lean_inc(v_dst_2718_);
v_src_2719_ = lean_ctor_get(v_x_2672_, 1);
lean_inc(v_src_2719_);
v_pd_2720_ = lean_ctor_get(v_x_2672_, 2);
lean_inc(v_pd_2720_);
v_ps_2721_ = lean_ctor_get(v_x_2672_, 3);
lean_inc(v_ps_2721_);
v_b_2722_ = lean_ctor_get(v_x_2672_, 4);
lean_inc(v_b_2722_);
lean_dec_ref_known(v_x_2672_, 5);
v___x_2723_ = lp_clauseflow_ClauseFlow_kill(v_pd_2720_, v_x_2673_);
v_a1_2724_ = lp_clauseflow_ClauseFlow_kill(v_dst_2718_, v___x_2723_);
v___x_2729_ = lean_nat_dec_eq(v_dst_2718_, v_pd_2720_);
v___x_2730_ = lean_nat_dec_eq(v_pd_2720_, v_ps_2721_);
v___x_2731_ = lean_nat_dec_eq(v_pd_2720_, v_src_2719_);
v___x_2772_ = lean_nat_dec_eq(v_dst_2718_, v_src_2719_);
if (v___x_2772_ == 0)
{
uint8_t v___x_2773_; 
v___x_2773_ = lean_nat_dec_eq(v_dst_2718_, v_ps_2721_);
v___y_2733_ = v___x_2773_;
goto v___jp_2732_;
}
else
{
v___y_2733_ = v___x_2772_;
goto v___jp_2732_;
}
v___jp_2725_:
{
lean_object* v___x_2726_; lean_object* v___x_2727_; lean_object* v___x_2728_; 
v___x_2726_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2726_, 0, v_b_2722_);
lean_ctor_set(v___x_2726_, 1, v_a1_2724_);
v___x_2727_ = lean_box(0);
v___x_2728_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2728_, 0, v___x_2726_);
lean_ctor_set(v___x_2728_, 1, v___x_2727_);
return v___x_2728_;
}
v___jp_2732_:
{
if (v___y_2733_ == 0)
{
if (v___x_2731_ == 0)
{
if (v___x_2730_ == 0)
{
if (v___x_2729_ == 0)
{
lean_object* v_facts_2734_; lean_object* v_cls_2735_; lean_object* v___x_2737_; uint8_t v_isShared_2738_; uint8_t v_isSharedCheck_2771_; 
v_facts_2734_ = lean_ctor_get(v_a1_2724_, 0);
v_cls_2735_ = lean_ctor_get(v_a1_2724_, 1);
v_isSharedCheck_2771_ = !lean_is_exclusive(v_a1_2724_);
if (v_isSharedCheck_2771_ == 0)
{
v___x_2737_ = v_a1_2724_;
v_isShared_2738_ = v_isSharedCheck_2771_;
goto v_resetjp_2736_;
}
else
{
lean_inc(v_cls_2735_);
lean_inc(v_facts_2734_);
lean_dec(v_a1_2724_);
v___x_2737_ = lean_box(0);
v_isShared_2738_ = v_isSharedCheck_2771_;
goto v_resetjp_2736_;
}
v_resetjp_2736_:
{
lean_object* v___x_2739_; lean_object* v___x_2740_; lean_object* v___x_2741_; lean_object* v___x_2742_; lean_object* v_moved_2743_; lean_object* v___x_2744_; lean_object* v___x_2745_; lean_object* v___x_2746_; lean_object* v___x_2747_; lean_object* v___x_2748_; lean_object* v___x_2749_; lean_object* v___x_2750_; lean_object* v___x_2751_; lean_object* v___x_2752_; lean_object* v___x_2753_; lean_object* v___x_2754_; lean_object* v___x_2755_; lean_object* v___x_2756_; lean_object* v___x_2757_; lean_object* v___x_2758_; lean_object* v___x_2759_; lean_object* v___x_2760_; lean_object* v___x_2761_; lean_object* v___x_2762_; lean_object* v___x_2763_; lean_object* v___x_2764_; lean_object* v___x_2765_; lean_object* v___x_2767_; 
v___x_2739_ = lean_box(0);
lean_inc(v_facts_2734_);
v___x_2740_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__4(v_facts_2734_, v___x_2739_);
lean_inc(v_cls_2735_);
v___x_2741_ = l_List_appendTR___redArg(v_cls_2735_, v___x_2740_);
v___x_2742_ = lp_clauseflow_List_filterTR_loop___at___00ClauseFlow_tTerm_spec__5(v_src_2719_, v_ps_2721_, v___x_2741_, v___x_2739_);
lean_inc_n(v_dst_2718_, 3);
lean_inc_n(v_src_2719_, 2);
v_moved_2743_ = lp_clauseflow_List_mapTR_loop___at___00ClauseFlow_tTerm_spec__6(v_src_2719_, v_dst_2718_, v_ps_2721_, v_pd_2720_, v___x_2742_, v___x_2739_);
lean_dec(v_ps_2721_);
v___x_2744_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__0));
v___x_2745_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2745_, 0, v_src_2719_);
lean_ctor_set(v___x_2745_, 1, v___x_2744_);
v___x_2746_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tStmt___closed__1));
v___x_2747_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2747_, 0, v_dst_2718_);
lean_ctor_set(v___x_2747_, 1, v___x_2746_);
v___x_2748_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2748_, 0, v___x_2747_);
lean_ctor_set(v___x_2748_, 1, v___x_2739_);
v___x_2749_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2749_, 0, v___x_2745_);
lean_ctor_set(v___x_2749_, 1, v___x_2748_);
v___x_2750_ = lean_box(0);
v___x_2751_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2751_, 0, v___x_2749_);
lean_ctor_set(v___x_2751_, 1, v___x_2750_);
v___x_2752_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2752_, 0, v_src_2719_);
lean_ctor_set(v___x_2752_, 1, v___x_2746_);
v___x_2753_ = ((lean_object*)(lp_clauseflow_ClauseFlow_tTerm___closed__0));
v___x_2754_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2754_, 0, v_dst_2718_);
lean_ctor_set(v___x_2754_, 1, v___x_2753_);
v___x_2755_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2755_, 0, v___x_2754_);
lean_ctor_set(v___x_2755_, 1, v___x_2739_);
lean_inc_ref(v___x_2752_);
v___x_2756_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2756_, 0, v___x_2752_);
lean_ctor_set(v___x_2756_, 1, v___x_2755_);
v___x_2757_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2757_, 0, v___x_2756_);
lean_ctor_set(v___x_2757_, 1, v___x_2750_);
v___x_2758_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2758_, 0, v_dst_2718_);
lean_ctor_set(v___x_2758_, 1, v___x_2744_);
v___x_2759_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2759_, 0, v___x_2752_);
lean_ctor_set(v___x_2759_, 1, v___x_2739_);
v___x_2760_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2760_, 0, v___x_2758_);
lean_ctor_set(v___x_2760_, 1, v___x_2759_);
v___x_2761_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2761_, 0, v___x_2760_);
lean_ctor_set(v___x_2761_, 1, v___x_2750_);
v___x_2762_ = l_List_appendTR___redArg(v_moved_2743_, v_cls_2735_);
v___x_2763_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2763_, 0, v___x_2761_);
lean_ctor_set(v___x_2763_, 1, v___x_2762_);
v___x_2764_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2764_, 0, v___x_2757_);
lean_ctor_set(v___x_2764_, 1, v___x_2763_);
v___x_2765_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2765_, 0, v___x_2751_);
lean_ctor_set(v___x_2765_, 1, v___x_2764_);
if (v_isShared_2738_ == 0)
{
lean_ctor_set(v___x_2737_, 1, v___x_2765_);
v___x_2767_ = v___x_2737_;
goto v_reusejp_2766_;
}
else
{
lean_object* v_reuseFailAlloc_2770_; 
v_reuseFailAlloc_2770_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v_reuseFailAlloc_2770_, 0, v_facts_2734_);
lean_ctor_set(v_reuseFailAlloc_2770_, 1, v___x_2765_);
v___x_2767_ = v_reuseFailAlloc_2770_;
goto v_reusejp_2766_;
}
v_reusejp_2766_:
{
lean_object* v___x_2768_; lean_object* v___x_2769_; 
v___x_2768_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2768_, 0, v_b_2722_);
lean_ctor_set(v___x_2768_, 1, v___x_2767_);
v___x_2769_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2769_, 0, v___x_2768_);
lean_ctor_set(v___x_2769_, 1, v___x_2739_);
return v___x_2769_;
}
}
}
else
{
lean_dec(v_ps_2721_);
lean_dec(v_pd_2720_);
lean_dec(v_src_2719_);
lean_dec(v_dst_2718_);
goto v___jp_2725_;
}
}
else
{
lean_dec(v_ps_2721_);
lean_dec(v_pd_2720_);
lean_dec(v_src_2719_);
lean_dec(v_dst_2718_);
goto v___jp_2725_;
}
}
else
{
lean_dec(v_ps_2721_);
lean_dec(v_pd_2720_);
lean_dec(v_src_2719_);
lean_dec(v_dst_2718_);
goto v___jp_2725_;
}
}
else
{
lean_dec(v_ps_2721_);
lean_dec(v_pd_2720_);
lean_dec(v_src_2719_);
lean_dec(v_dst_2718_);
goto v___jp_2725_;
}
}
}
default: 
{
lean_object* v___x_2774_; 
lean_dec_ref(v_x_2673_);
lean_dec(v_x_2672_);
v___x_2774_ = lean_box(0);
return v___x_2774_;
}
}
}
}
static lean_object* _init_lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__4(void){
_start:
{
lean_object* v___x_2784_; lean_object* v___x_2785_; 
v___x_2784_ = lean_unsigned_to_nat(10u);
v___x_2785_ = lean_nat_to_int(v___x_2784_);
return v___x_2785_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg(lean_object* v_x_2789_){
_start:
{
lean_object* v_cls_2790_; lean_object* v_exempt_2791_; lean_object* v_init_2792_; lean_object* v___x_2793_; lean_object* v___x_2794_; lean_object* v___x_2795_; lean_object* v___x_2796_; lean_object* v___x_2797_; uint8_t v___x_2798_; lean_object* v___x_2799_; lean_object* v___x_2800_; lean_object* v___x_2801_; lean_object* v___x_2802_; lean_object* v___x_2803_; lean_object* v___x_2804_; lean_object* v___x_2805_; lean_object* v___x_2806_; lean_object* v___x_2807_; lean_object* v___x_2808_; lean_object* v___x_2809_; lean_object* v___x_2810_; lean_object* v___x_2811_; lean_object* v___x_2812_; lean_object* v___x_2813_; lean_object* v___x_2814_; lean_object* v___x_2815_; lean_object* v___x_2816_; lean_object* v___x_2817_; lean_object* v___x_2818_; lean_object* v___x_2819_; lean_object* v___x_2820_; lean_object* v___x_2821_; lean_object* v___x_2822_; lean_object* v___x_2823_; lean_object* v___x_2824_; lean_object* v___x_2825_; lean_object* v___x_2826_; lean_object* v___x_2827_; lean_object* v___x_2828_; lean_object* v___x_2829_; 
v_cls_2790_ = lean_ctor_get(v_x_2789_, 0);
lean_inc(v_cls_2790_);
v_exempt_2791_ = lean_ctor_get(v_x_2789_, 1);
lean_inc(v_exempt_2791_);
v_init_2792_ = lean_ctor_get(v_x_2789_, 2);
lean_inc(v_init_2792_);
lean_dec_ref(v_x_2789_);
v___x_2793_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__5));
v___x_2794_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__1));
v___x_2795_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6, &lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6_once, _init_lp_clauseflow_ClauseFlow_instReprAState_repr___redArg___closed__6);
v___x_2796_ = lp_clauseflow_List_repr___at___00ClauseFlow_instReprTerm_repr_spec__1___redArg(v_cls_2790_);
v___x_2797_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_2797_, 0, v___x_2795_);
lean_ctor_set(v___x_2797_, 1, v___x_2796_);
v___x_2798_ = 0;
v___x_2799_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_2799_, 0, v___x_2797_);
lean_ctor_set_uint8(v___x_2799_, sizeof(void*)*1, v___x_2798_);
v___x_2800_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2800_, 0, v___x_2794_);
lean_ctor_set(v___x_2800_, 1, v___x_2799_);
v___x_2801_ = ((lean_object*)(lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg___closed__4));
v___x_2802_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2802_, 0, v___x_2800_);
lean_ctor_set(v___x_2802_, 1, v___x_2801_);
v___x_2803_ = lean_box(1);
v___x_2804_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2804_, 0, v___x_2802_);
lean_ctor_set(v___x_2804_, 1, v___x_2803_);
v___x_2805_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__3));
v___x_2806_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2806_, 0, v___x_2804_);
lean_ctor_set(v___x_2806_, 1, v___x_2805_);
v___x_2807_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2807_, 0, v___x_2806_);
lean_ctor_set(v___x_2807_, 1, v___x_2793_);
v___x_2808_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__4, &lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__4_once, _init_lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__4);
v___x_2809_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_exempt_2791_);
v___x_2810_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_2810_, 0, v___x_2808_);
lean_ctor_set(v___x_2810_, 1, v___x_2809_);
v___x_2811_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_2811_, 0, v___x_2810_);
lean_ctor_set_uint8(v___x_2811_, sizeof(void*)*1, v___x_2798_);
v___x_2812_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2812_, 0, v___x_2807_);
lean_ctor_set(v___x_2812_, 1, v___x_2811_);
v___x_2813_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2813_, 0, v___x_2812_);
lean_ctor_set(v___x_2813_, 1, v___x_2801_);
v___x_2814_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2814_, 0, v___x_2813_);
lean_ctor_set(v___x_2814_, 1, v___x_2803_);
v___x_2815_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg___closed__6));
v___x_2816_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2816_, 0, v___x_2814_);
lean_ctor_set(v___x_2816_, 1, v___x_2815_);
v___x_2817_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2817_, 0, v___x_2816_);
lean_ctor_set(v___x_2817_, 1, v___x_2793_);
v___x_2818_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7, &lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7_once, _init_lp_clauseflow_ClauseFlow_instReprBlockD_repr___redArg___closed__7);
v___x_2819_ = lp_clauseflow_List_repr_x27___at___00ClauseFlow_instReprConcl_repr_spec__0___redArg(v_init_2792_);
v___x_2820_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_2820_, 0, v___x_2818_);
lean_ctor_set(v___x_2820_, 1, v___x_2819_);
v___x_2821_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_2821_, 0, v___x_2820_);
lean_ctor_set_uint8(v___x_2821_, sizeof(void*)*1, v___x_2798_);
v___x_2822_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2822_, 0, v___x_2817_);
lean_ctor_set(v___x_2822_, 1, v___x_2821_);
v___x_2823_ = lean_obj_once(&lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13, &lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13_once, _init_lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__13);
v___x_2824_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__14));
v___x_2825_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2825_, 0, v___x_2824_);
lean_ctor_set(v___x_2825_, 1, v___x_2822_);
v___x_2826_ = ((lean_object*)(lp_clauseflow_ClauseFlow_instReprClause_repr___redArg___closed__15));
v___x_2827_ = lean_alloc_ctor(5, 2, 0);
lean_ctor_set(v___x_2827_, 0, v___x_2825_);
lean_ctor_set(v___x_2827_, 1, v___x_2826_);
v___x_2828_ = lean_alloc_ctor(4, 2, 0);
lean_ctor_set(v___x_2828_, 0, v___x_2823_);
lean_ctor_set(v___x_2828_, 1, v___x_2827_);
v___x_2829_ = lean_alloc_ctor(6, 1, 1);
lean_ctor_set(v___x_2829_, 0, v___x_2828_);
lean_ctor_set_uint8(v___x_2829_, sizeof(void*)*1, v___x_2798_);
return v___x_2829_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr(lean_object* v_x_2830_, lean_object* v_prec_2831_){
_start:
{
lean_object* v___x_2832_; 
v___x_2832_ = lp_clauseflow_ClauseFlow_instReprGoal_repr___redArg(v_x_2830_);
return v___x_2832_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_instReprGoal_repr___boxed(lean_object* v_x_2833_, lean_object* v_prec_2834_){
_start:
{
lean_object* v_res_2835_; 
v_res_2835_ = lp_clauseflow_ClauseFlow_instReprGoal_repr(v_x_2833_, v_prec_2834_);
lean_dec(v_prec_2834_);
return v_res_2835_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_edgeOk(lean_object* v_inv_2838_, lean_object* v_e_2839_){
_start:
{
lean_object* v_snd_2840_; lean_object* v_fst_2841_; lean_object* v_facts_2842_; lean_object* v___x_2843_; uint8_t v___x_2844_; 
v_snd_2840_ = lean_ctor_get(v_e_2839_, 1);
lean_inc_n(v_snd_2840_, 2);
v_fst_2841_ = lean_ctor_get(v_e_2839_, 0);
lean_inc(v_fst_2841_);
lean_dec_ref(v_e_2839_);
v_facts_2842_ = lean_ctor_get(v_snd_2840_, 0);
lean_inc(v_facts_2842_);
v___x_2843_ = lp_clauseflow_ClauseFlow_closure(v_snd_2840_, v_facts_2842_);
v___x_2844_ = lp_clauseflow_List_any___at___00ClauseFlow_inconsistent_spec__1(v___x_2843_, v___x_2843_);
lean_dec(v___x_2843_);
if (v___x_2844_ == 0)
{
lean_object* v___x_2845_; 
v___x_2845_ = l_List_get_x3fInternal___redArg(v_inv_2838_, v_fst_2841_);
if (lean_obj_tag(v___x_2845_) == 1)
{
lean_object* v_val_2846_; 
v_val_2846_ = lean_ctor_get(v___x_2845_, 0);
lean_inc(v_val_2846_);
lean_dec_ref_known(v___x_2845_, 1);
if (lean_obj_tag(v_val_2846_) == 1)
{
lean_object* v_val_2847_; uint8_t v___x_2848_; 
v_val_2847_ = lean_ctor_get(v_val_2846_, 0);
lean_inc(v_val_2847_);
lean_dec_ref_known(v_val_2846_, 1);
v___x_2848_ = lp_clauseflow_ClauseFlow_le(v_snd_2840_, v_val_2847_);
return v___x_2848_;
}
else
{
lean_dec(v_val_2846_);
lean_dec(v_snd_2840_);
return v___x_2844_;
}
}
else
{
lean_dec(v___x_2845_);
lean_dec(v_snd_2840_);
return v___x_2844_;
}
}
else
{
lean_dec(v_fst_2841_);
lean_dec(v_snd_2840_);
return v___x_2844_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_edgeOk___boxed(lean_object* v_inv_2849_, lean_object* v_e_2850_){
_start:
{
uint8_t v_res_2851_; lean_object* v_r_2852_; 
v_res_2851_ = lp_clauseflow_ClauseFlow_edgeOk(v_inv_2849_, v_e_2850_);
lean_dec(v_inv_2849_);
v_r_2852_ = lean_box(v_res_2851_);
return v_r_2852_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_blockOk_spec__0(lean_object* v_inv_2853_, lean_object* v_x_2854_){
_start:
{
if (lean_obj_tag(v_x_2854_) == 0)
{
uint8_t v___x_2855_; 
v___x_2855_ = 1;
return v___x_2855_;
}
else
{
lean_object* v_head_2856_; lean_object* v_tail_2857_; uint8_t v___x_2858_; 
v_head_2856_ = lean_ctor_get(v_x_2854_, 0);
lean_inc(v_head_2856_);
v_tail_2857_ = lean_ctor_get(v_x_2854_, 1);
lean_inc(v_tail_2857_);
lean_dec_ref_known(v_x_2854_, 2);
v___x_2858_ = lp_clauseflow_ClauseFlow_edgeOk(v_inv_2853_, v_head_2856_);
if (v___x_2858_ == 0)
{
lean_dec(v_tail_2857_);
return v___x_2858_;
}
else
{
v_x_2854_ = v_tail_2857_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_blockOk_spec__0___boxed(lean_object* v_inv_2860_, lean_object* v_x_2861_){
_start:
{
uint8_t v_res_2862_; lean_object* v_r_2863_; 
v_res_2862_ = lp_clauseflow_List_all___at___00ClauseFlow_blockOk_spec__0(v_inv_2860_, v_x_2861_);
lean_dec(v_inv_2860_);
v_r_2863_ = lean_box(v_res_2862_);
return v_r_2863_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_blockOk(lean_object* v_g_2864_, lean_object* v_cfg_2865_, lean_object* v_inv_2866_, lean_object* v_b_2867_){
_start:
{
lean_object* v_cls_2868_; lean_object* v_exempt_2869_; lean_object* v___f_2870_; uint8_t v___x_2871_; 
v_cls_2868_ = lean_ctor_get(v_g_2864_, 0);
lean_inc(v_cls_2868_);
v_exempt_2869_ = lean_ctor_get(v_g_2864_, 1);
lean_inc(v_exempt_2869_);
lean_dec_ref(v_g_2864_);
v___f_2870_ = lean_obj_once(&lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0, &lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0_once, _init_lp_clauseflow_List_any___at___00ClauseFlow_entailsCl_spec__1___closed__0);
lean_inc(v_b_2867_);
v___x_2871_ = l_List_elem___redArg(v___f_2870_, v_b_2867_, v_exempt_2869_);
if (v___x_2871_ == 0)
{
lean_object* v___x_2872_; 
lean_inc(v_b_2867_);
v___x_2872_ = l_List_get_x3fInternal___redArg(v_cfg_2865_, v_b_2867_);
if (lean_obj_tag(v___x_2872_) == 1)
{
lean_object* v_val_2873_; lean_object* v___x_2874_; 
v_val_2873_ = lean_ctor_get(v___x_2872_, 0);
lean_inc(v_val_2873_);
lean_dec_ref_known(v___x_2872_, 1);
v___x_2874_ = l_List_get_x3fInternal___redArg(v_inv_2866_, v_b_2867_);
if (lean_obj_tag(v___x_2874_) == 1)
{
lean_object* v_val_2875_; 
v_val_2875_ = lean_ctor_get(v___x_2874_, 0);
lean_inc(v_val_2875_);
lean_dec_ref_known(v___x_2874_, 1);
if (lean_obj_tag(v_val_2875_) == 1)
{
lean_object* v_val_2876_; lean_object* v_stmts_2877_; lean_object* v_term_2878_; lean_object* v_a1_2879_; lean_object* v___x_2880_; uint8_t v___x_2881_; 
v_val_2876_ = lean_ctor_get(v_val_2875_, 0);
lean_inc(v_val_2876_);
lean_dec_ref_known(v_val_2875_, 1);
v_stmts_2877_ = lean_ctor_get(v_val_2873_, 0);
lean_inc(v_stmts_2877_);
v_term_2878_ = lean_ctor_get(v_val_2873_, 1);
lean_inc_n(v_term_2878_, 2);
lean_dec(v_val_2873_);
v_a1_2879_ = lp_clauseflow_ClauseFlow_tStmts(v_stmts_2877_, v_val_2876_);
lean_inc_ref(v_a1_2879_);
v___x_2880_ = lp_clauseflow_ClauseFlow_tTerm(v_term_2878_, v_a1_2879_);
v___x_2881_ = lp_clauseflow_List_all___at___00ClauseFlow_blockOk_spec__0(v_inv_2866_, v___x_2880_);
if (v___x_2881_ == 0)
{
lean_dec_ref(v_a1_2879_);
lean_dec(v_term_2878_);
lean_dec(v_cls_2868_);
return v___x_2881_;
}
else
{
if (lean_obj_tag(v_term_2878_) == 4)
{
uint8_t v___x_2882_; 
v___x_2882_ = lp_clauseflow_List_all___at___00ClauseFlow_le_spec__1(v_a1_2879_, v_cls_2868_);
return v___x_2882_;
}
else
{
lean_dec_ref(v_a1_2879_);
lean_dec(v_term_2878_);
lean_dec(v_cls_2868_);
return v___x_2881_;
}
}
}
else
{
uint8_t v___x_2883_; 
lean_dec(v_val_2875_);
lean_dec(v_val_2873_);
lean_dec(v_cls_2868_);
v___x_2883_ = 1;
return v___x_2883_;
}
}
else
{
uint8_t v___x_2884_; 
lean_dec(v___x_2874_);
lean_dec(v_val_2873_);
lean_dec(v_cls_2868_);
v___x_2884_ = 1;
return v___x_2884_;
}
}
else
{
uint8_t v___x_2885_; 
lean_dec(v___x_2872_);
lean_dec(v_cls_2868_);
lean_dec(v_b_2867_);
v___x_2885_ = 1;
return v___x_2885_;
}
}
else
{
lean_dec(v_cls_2868_);
lean_dec(v_b_2867_);
return v___x_2871_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_blockOk___boxed(lean_object* v_g_2886_, lean_object* v_cfg_2887_, lean_object* v_inv_2888_, lean_object* v_b_2889_){
_start:
{
uint8_t v_res_2890_; lean_object* v_r_2891_; 
v_res_2890_ = lp_clauseflow_ClauseFlow_blockOk(v_g_2886_, v_cfg_2887_, v_inv_2888_, v_b_2889_);
lean_dec(v_inv_2888_);
lean_dec(v_cfg_2887_);
v_r_2891_ = lean_box(v_res_2890_);
return v_r_2891_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_initState(lean_object* v_g_2892_){
_start:
{
lean_object* v_init_2893_; lean_object* v___x_2894_; lean_object* v___x_2895_; lean_object* v___x_2896_; lean_object* v___x_2897_; lean_object* v___x_2898_; 
v_init_2893_ = lean_ctor_get(v_g_2892_, 2);
v___x_2894_ = lean_box(0);
lean_inc(v_init_2893_);
v___x_2895_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2895_, 0, v___x_2894_);
lean_ctor_set(v___x_2895_, 1, v_init_2893_);
v___x_2896_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2896_, 0, v___x_2894_);
lean_ctor_set(v___x_2896_, 1, v___x_2895_);
v___x_2897_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_2897_, 0, v___x_2896_);
lean_ctor_set(v___x_2897_, 1, v___x_2894_);
v___x_2898_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_2898_, 0, v___x_2894_);
lean_ctor_set(v___x_2898_, 1, v___x_2897_);
return v___x_2898_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_initState___boxed(lean_object* v_g_2899_){
_start:
{
lean_object* v_res_2900_; 
v_res_2900_ = lp_clauseflow_ClauseFlow_initState(v_g_2899_);
lean_dec_ref(v_g_2899_);
return v_res_2900_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_List_all___at___00ClauseFlow_checkCert_spec__0(lean_object* v_g_2901_, lean_object* v_cfg_2902_, lean_object* v_inv_2903_, lean_object* v_x_2904_){
_start:
{
if (lean_obj_tag(v_x_2904_) == 0)
{
uint8_t v___x_2905_; 
lean_dec_ref(v_g_2901_);
v___x_2905_ = 1;
return v___x_2905_;
}
else
{
lean_object* v_head_2906_; lean_object* v_tail_2907_; uint8_t v___x_2908_; 
v_head_2906_ = lean_ctor_get(v_x_2904_, 0);
lean_inc(v_head_2906_);
v_tail_2907_ = lean_ctor_get(v_x_2904_, 1);
lean_inc(v_tail_2907_);
lean_dec_ref_known(v_x_2904_, 2);
lean_inc_ref(v_g_2901_);
v___x_2908_ = lp_clauseflow_ClauseFlow_blockOk(v_g_2901_, v_cfg_2902_, v_inv_2903_, v_head_2906_);
if (v___x_2908_ == 0)
{
lean_dec(v_tail_2907_);
lean_dec_ref(v_g_2901_);
return v___x_2908_;
}
else
{
v_x_2904_ = v_tail_2907_;
goto _start;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_List_all___at___00ClauseFlow_checkCert_spec__0___boxed(lean_object* v_g_2910_, lean_object* v_cfg_2911_, lean_object* v_inv_2912_, lean_object* v_x_2913_){
_start:
{
uint8_t v_res_2914_; lean_object* v_r_2915_; 
v_res_2914_ = lp_clauseflow_List_all___at___00ClauseFlow_checkCert_spec__0(v_g_2910_, v_cfg_2911_, v_inv_2912_, v_x_2913_);
lean_dec(v_inv_2912_);
lean_dec(v_cfg_2911_);
v_r_2915_ = lean_box(v_res_2914_);
return v_r_2915_;
}
}
LEAN_EXPORT uint8_t lp_clauseflow_ClauseFlow_checkCert(lean_object* v_g_2916_, lean_object* v_cfg_2917_, lean_object* v_inv_2918_){
_start:
{
lean_object* v___x_2919_; lean_object* v___x_2920_; 
v___x_2919_ = lean_unsigned_to_nat(0u);
v___x_2920_ = l_List_get_x3fInternal___redArg(v_inv_2918_, v___x_2919_);
if (lean_obj_tag(v___x_2920_) == 1)
{
lean_object* v_val_2921_; 
v_val_2921_ = lean_ctor_get(v___x_2920_, 0);
lean_inc(v_val_2921_);
lean_dec_ref_known(v___x_2920_, 1);
if (lean_obj_tag(v_val_2921_) == 1)
{
lean_object* v_val_2922_; lean_object* v___x_2923_; uint8_t v___x_2924_; 
v_val_2922_ = lean_ctor_get(v_val_2921_, 0);
lean_inc(v_val_2922_);
lean_dec_ref_known(v_val_2921_, 1);
v___x_2923_ = lp_clauseflow_ClauseFlow_initState(v_g_2916_);
v___x_2924_ = lp_clauseflow_ClauseFlow_le(v___x_2923_, v_val_2922_);
if (v___x_2924_ == 0)
{
lean_dec_ref(v_g_2916_);
return v___x_2924_;
}
else
{
lean_object* v___x_2925_; lean_object* v___x_2926_; uint8_t v___x_2927_; 
v___x_2925_ = l_List_lengthTR___redArg(v_cfg_2917_);
v___x_2926_ = l_List_range(v___x_2925_);
v___x_2927_ = lp_clauseflow_List_all___at___00ClauseFlow_checkCert_spec__0(v_g_2916_, v_cfg_2917_, v_inv_2918_, v___x_2926_);
return v___x_2927_;
}
}
else
{
uint8_t v___x_2928_; 
lean_dec(v_val_2921_);
lean_dec_ref(v_g_2916_);
v___x_2928_ = 0;
return v___x_2928_;
}
}
else
{
uint8_t v___x_2929_; 
lean_dec(v___x_2920_);
lean_dec_ref(v_g_2916_);
v___x_2929_ = 0;
return v___x_2929_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow_ClauseFlow_checkCert___boxed(lean_object* v_g_2930_, lean_object* v_cfg_2931_, lean_object* v_inv_2932_){
_start:
{
uint8_t v_res_2933_; lean_object* v_r_2934_; 
v_res_2933_ = lp_clauseflow_ClauseFlow_checkCert(v_g_2930_, v_cfg_2931_, v_inv_2932_);
lean_dec(v_inv_2932_);
lean_dec(v_cfg_2931_);
v_r_2934_ = lean_box(v_res_2933_);
return v_r_2934_;
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprTest_repr_match__1_splitter___redArg(lean_object* v_x_2935_, lean_object* v_h__1_2936_, lean_object* v_h__2_2937_){
_start:
{
if (lean_obj_tag(v_x_2935_) == 0)
{
lean_object* v_n_2938_; lean_object* v___x_2939_; 
lean_dec(v_h__2_2937_);
v_n_2938_ = lean_ctor_get(v_x_2935_, 0);
lean_inc(v_n_2938_);
lean_dec_ref_known(v_x_2935_, 1);
v___x_2939_ = lean_apply_1(v_h__1_2936_, v_n_2938_);
return v___x_2939_;
}
else
{
lean_object* v_n_2940_; lean_object* v___x_2941_; 
lean_dec(v_h__1_2936_);
v_n_2940_ = lean_ctor_get(v_x_2935_, 0);
lean_inc(v_n_2940_);
lean_dec_ref_known(v_x_2935_, 1);
v___x_2941_ = lean_apply_1(v_h__2_2937_, v_n_2940_);
return v___x_2941_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprTest_repr_match__1_splitter(lean_object* v_motive_2942_, lean_object* v_x_2943_, lean_object* v_h__1_2944_, lean_object* v_h__2_2945_){
_start:
{
if (lean_obj_tag(v_x_2943_) == 0)
{
lean_object* v_n_2946_; lean_object* v___x_2947_; 
lean_dec(v_h__2_2945_);
v_n_2946_ = lean_ctor_get(v_x_2943_, 0);
lean_inc(v_n_2946_);
lean_dec_ref_known(v_x_2943_, 1);
v___x_2947_ = lean_apply_1(v_h__1_2944_, v_n_2946_);
return v___x_2947_;
}
else
{
lean_object* v_n_2948_; lean_object* v___x_2949_; 
lean_dec(v_h__1_2944_);
v_n_2948_ = lean_ctor_get(v_x_2943_, 0);
lean_inc(v_n_2948_);
lean_dec_ref_known(v_x_2943_, 1);
v___x_2949_ = lean_apply_1(v_h__2_2945_, v_n_2948_);
return v___x_2949_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instDecidableEqTest_decEq_match__1_splitter___redArg(lean_object* v_x_2950_, lean_object* v_x_2951_, lean_object* v_h__1_2952_, lean_object* v_h__2_2953_, lean_object* v_h__3_2954_, lean_object* v_h__4_2955_){
_start:
{
if (lean_obj_tag(v_x_2950_) == 0)
{
lean_dec(v_h__4_2955_);
lean_dec(v_h__3_2954_);
if (lean_obj_tag(v_x_2951_) == 0)
{
lean_object* v_n_2956_; lean_object* v_n_2957_; lean_object* v___x_2958_; 
lean_dec(v_h__2_2953_);
v_n_2956_ = lean_ctor_get(v_x_2950_, 0);
lean_inc(v_n_2956_);
lean_dec_ref_known(v_x_2950_, 1);
v_n_2957_ = lean_ctor_get(v_x_2951_, 0);
lean_inc(v_n_2957_);
lean_dec_ref_known(v_x_2951_, 1);
v___x_2958_ = lean_apply_2(v_h__1_2952_, v_n_2956_, v_n_2957_);
return v___x_2958_;
}
else
{
lean_object* v_n_2959_; lean_object* v_n_2960_; lean_object* v___x_2961_; 
lean_dec(v_h__1_2952_);
v_n_2959_ = lean_ctor_get(v_x_2950_, 0);
lean_inc(v_n_2959_);
lean_dec_ref_known(v_x_2950_, 1);
v_n_2960_ = lean_ctor_get(v_x_2951_, 0);
lean_inc(v_n_2960_);
lean_dec_ref_known(v_x_2951_, 1);
v___x_2961_ = lean_apply_2(v_h__2_2953_, v_n_2959_, v_n_2960_);
return v___x_2961_;
}
}
else
{
lean_dec(v_h__2_2953_);
lean_dec(v_h__1_2952_);
if (lean_obj_tag(v_x_2951_) == 0)
{
lean_object* v_n_2962_; lean_object* v_n_2963_; lean_object* v___x_2964_; 
lean_dec(v_h__4_2955_);
v_n_2962_ = lean_ctor_get(v_x_2950_, 0);
lean_inc(v_n_2962_);
lean_dec_ref_known(v_x_2950_, 1);
v_n_2963_ = lean_ctor_get(v_x_2951_, 0);
lean_inc(v_n_2963_);
lean_dec_ref_known(v_x_2951_, 1);
v___x_2964_ = lean_apply_2(v_h__3_2954_, v_n_2962_, v_n_2963_);
return v___x_2964_;
}
else
{
lean_object* v_n_2965_; lean_object* v_n_2966_; lean_object* v___x_2967_; 
lean_dec(v_h__3_2954_);
v_n_2965_ = lean_ctor_get(v_x_2950_, 0);
lean_inc(v_n_2965_);
lean_dec_ref_known(v_x_2950_, 1);
v_n_2966_ = lean_ctor_get(v_x_2951_, 0);
lean_inc(v_n_2966_);
lean_dec_ref_known(v_x_2951_, 1);
v___x_2967_ = lean_apply_2(v_h__4_2955_, v_n_2965_, v_n_2966_);
return v___x_2967_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instDecidableEqTest_decEq_match__1_splitter(lean_object* v_motive_2968_, lean_object* v_x_2969_, lean_object* v_x_2970_, lean_object* v_h__1_2971_, lean_object* v_h__2_2972_, lean_object* v_h__3_2973_, lean_object* v_h__4_2974_){
_start:
{
if (lean_obj_tag(v_x_2969_) == 0)
{
lean_dec(v_h__4_2974_);
lean_dec(v_h__3_2973_);
if (lean_obj_tag(v_x_2970_) == 0)
{
lean_object* v_n_2975_; lean_object* v_n_2976_; lean_object* v___x_2977_; 
lean_dec(v_h__2_2972_);
v_n_2975_ = lean_ctor_get(v_x_2969_, 0);
lean_inc(v_n_2975_);
lean_dec_ref_known(v_x_2969_, 1);
v_n_2976_ = lean_ctor_get(v_x_2970_, 0);
lean_inc(v_n_2976_);
lean_dec_ref_known(v_x_2970_, 1);
v___x_2977_ = lean_apply_2(v_h__1_2971_, v_n_2975_, v_n_2976_);
return v___x_2977_;
}
else
{
lean_object* v_n_2978_; lean_object* v_n_2979_; lean_object* v___x_2980_; 
lean_dec(v_h__1_2971_);
v_n_2978_ = lean_ctor_get(v_x_2969_, 0);
lean_inc(v_n_2978_);
lean_dec_ref_known(v_x_2969_, 1);
v_n_2979_ = lean_ctor_get(v_x_2970_, 0);
lean_inc(v_n_2979_);
lean_dec_ref_known(v_x_2970_, 1);
v___x_2980_ = lean_apply_2(v_h__2_2972_, v_n_2978_, v_n_2979_);
return v___x_2980_;
}
}
else
{
lean_dec(v_h__2_2972_);
lean_dec(v_h__1_2971_);
if (lean_obj_tag(v_x_2970_) == 0)
{
lean_object* v_n_2981_; lean_object* v_n_2982_; lean_object* v___x_2983_; 
lean_dec(v_h__4_2974_);
v_n_2981_ = lean_ctor_get(v_x_2969_, 0);
lean_inc(v_n_2981_);
lean_dec_ref_known(v_x_2969_, 1);
v_n_2982_ = lean_ctor_get(v_x_2970_, 0);
lean_inc(v_n_2982_);
lean_dec_ref_known(v_x_2970_, 1);
v___x_2983_ = lean_apply_2(v_h__3_2973_, v_n_2981_, v_n_2982_);
return v___x_2983_;
}
else
{
lean_object* v_n_2984_; lean_object* v_n_2985_; lean_object* v___x_2986_; 
lean_dec(v_h__3_2973_);
v_n_2984_ = lean_ctor_get(v_x_2969_, 0);
lean_inc(v_n_2984_);
lean_dec_ref_known(v_x_2969_, 1);
v_n_2985_ = lean_ctor_get(v_x_2970_, 0);
lean_inc(v_n_2985_);
lean_dec_ref_known(v_x_2970_, 1);
v___x_2986_ = lean_apply_2(v_h__4_2974_, v_n_2984_, v_n_2985_);
return v___x_2986_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_imp_match__1_splitter___redArg(lean_object* v_x_2987_, lean_object* v_x_2988_, lean_object* v_h__1_2989_, lean_object* v_h__2_2990_, lean_object* v_h__3_2991_, lean_object* v_h__4_2992_){
_start:
{
if (lean_obj_tag(v_x_2987_) == 0)
{
lean_dec(v_h__4_2992_);
lean_dec(v_h__3_2991_);
if (lean_obj_tag(v_x_2988_) == 0)
{
lean_object* v_n_2993_; lean_object* v_n_2994_; lean_object* v___x_2995_; 
lean_dec(v_h__2_2990_);
v_n_2993_ = lean_ctor_get(v_x_2987_, 0);
lean_inc(v_n_2993_);
lean_dec_ref_known(v_x_2987_, 1);
v_n_2994_ = lean_ctor_get(v_x_2988_, 0);
lean_inc(v_n_2994_);
lean_dec_ref_known(v_x_2988_, 1);
v___x_2995_ = lean_apply_2(v_h__1_2989_, v_n_2993_, v_n_2994_);
return v___x_2995_;
}
else
{
lean_object* v_n_2996_; lean_object* v_n_2997_; lean_object* v___x_2998_; 
lean_dec(v_h__1_2989_);
v_n_2996_ = lean_ctor_get(v_x_2987_, 0);
lean_inc(v_n_2996_);
lean_dec_ref_known(v_x_2987_, 1);
v_n_2997_ = lean_ctor_get(v_x_2988_, 0);
lean_inc(v_n_2997_);
lean_dec_ref_known(v_x_2988_, 1);
v___x_2998_ = lean_apply_2(v_h__2_2990_, v_n_2996_, v_n_2997_);
return v___x_2998_;
}
}
else
{
lean_dec(v_h__2_2990_);
lean_dec(v_h__1_2989_);
if (lean_obj_tag(v_x_2988_) == 0)
{
lean_object* v_n_2999_; lean_object* v_n_3000_; lean_object* v___x_3001_; 
lean_dec(v_h__3_2991_);
v_n_2999_ = lean_ctor_get(v_x_2987_, 0);
lean_inc(v_n_2999_);
lean_dec_ref_known(v_x_2987_, 1);
v_n_3000_ = lean_ctor_get(v_x_2988_, 0);
lean_inc(v_n_3000_);
lean_dec_ref_known(v_x_2988_, 1);
v___x_3001_ = lean_apply_2(v_h__4_2992_, v_n_2999_, v_n_3000_);
return v___x_3001_;
}
else
{
lean_object* v_n_3002_; lean_object* v_n_3003_; lean_object* v___x_3004_; 
lean_dec(v_h__4_2992_);
v_n_3002_ = lean_ctor_get(v_x_2987_, 0);
lean_inc(v_n_3002_);
lean_dec_ref_known(v_x_2987_, 1);
v_n_3003_ = lean_ctor_get(v_x_2988_, 0);
lean_inc(v_n_3003_);
lean_dec_ref_known(v_x_2988_, 1);
v___x_3004_ = lean_apply_2(v_h__3_2991_, v_n_3002_, v_n_3003_);
return v___x_3004_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_Test_imp_match__1_splitter(lean_object* v_motive_3005_, lean_object* v_x_3006_, lean_object* v_x_3007_, lean_object* v_h__1_3008_, lean_object* v_h__2_3009_, lean_object* v_h__3_3010_, lean_object* v_h__4_3011_){
_start:
{
if (lean_obj_tag(v_x_3006_) == 0)
{
lean_dec(v_h__4_3011_);
lean_dec(v_h__3_3010_);
if (lean_obj_tag(v_x_3007_) == 0)
{
lean_object* v_n_3012_; lean_object* v_n_3013_; lean_object* v___x_3014_; 
lean_dec(v_h__2_3009_);
v_n_3012_ = lean_ctor_get(v_x_3006_, 0);
lean_inc(v_n_3012_);
lean_dec_ref_known(v_x_3006_, 1);
v_n_3013_ = lean_ctor_get(v_x_3007_, 0);
lean_inc(v_n_3013_);
lean_dec_ref_known(v_x_3007_, 1);
v___x_3014_ = lean_apply_2(v_h__1_3008_, v_n_3012_, v_n_3013_);
return v___x_3014_;
}
else
{
lean_object* v_n_3015_; lean_object* v_n_3016_; lean_object* v___x_3017_; 
lean_dec(v_h__1_3008_);
v_n_3015_ = lean_ctor_get(v_x_3006_, 0);
lean_inc(v_n_3015_);
lean_dec_ref_known(v_x_3006_, 1);
v_n_3016_ = lean_ctor_get(v_x_3007_, 0);
lean_inc(v_n_3016_);
lean_dec_ref_known(v_x_3007_, 1);
v___x_3017_ = lean_apply_2(v_h__2_3009_, v_n_3015_, v_n_3016_);
return v___x_3017_;
}
}
else
{
lean_dec(v_h__2_3009_);
lean_dec(v_h__1_3008_);
if (lean_obj_tag(v_x_3007_) == 0)
{
lean_object* v_n_3018_; lean_object* v_n_3019_; lean_object* v___x_3020_; 
lean_dec(v_h__3_3010_);
v_n_3018_ = lean_ctor_get(v_x_3006_, 0);
lean_inc(v_n_3018_);
lean_dec_ref_known(v_x_3006_, 1);
v_n_3019_ = lean_ctor_get(v_x_3007_, 0);
lean_inc(v_n_3019_);
lean_dec_ref_known(v_x_3007_, 1);
v___x_3020_ = lean_apply_2(v_h__4_3011_, v_n_3018_, v_n_3019_);
return v___x_3020_;
}
else
{
lean_object* v_n_3021_; lean_object* v_n_3022_; lean_object* v___x_3023_; 
lean_dec(v_h__4_3011_);
v_n_3021_ = lean_ctor_get(v_x_3006_, 0);
lean_inc(v_n_3021_);
lean_dec_ref_known(v_x_3006_, 1);
v_n_3022_ = lean_ctor_get(v_x_3007_, 0);
lean_inc(v_n_3022_);
lean_dec_ref_known(v_x_3007_, 1);
v___x_3023_ = lean_apply_2(v_h__3_3010_, v_n_3021_, v_n_3022_);
return v___x_3023_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_lookupCase_match__1_splitter___redArg(lean_object* v_x_3024_, lean_object* v_x_3025_, lean_object* v_h__1_3026_, lean_object* v_h__2_3027_){
_start:
{
if (lean_obj_tag(v_x_3024_) == 0)
{
lean_object* v___x_3028_; 
lean_dec(v_h__2_3027_);
v___x_3028_ = lean_apply_1(v_h__1_3026_, v_x_3025_);
return v___x_3028_;
}
else
{
lean_object* v_head_3029_; lean_object* v_tail_3030_; lean_object* v_fst_3031_; lean_object* v_snd_3032_; lean_object* v___x_3033_; 
lean_dec(v_h__1_3026_);
v_head_3029_ = lean_ctor_get(v_x_3024_, 0);
lean_inc(v_head_3029_);
v_tail_3030_ = lean_ctor_get(v_x_3024_, 1);
lean_inc(v_tail_3030_);
lean_dec_ref_known(v_x_3024_, 2);
v_fst_3031_ = lean_ctor_get(v_head_3029_, 0);
lean_inc(v_fst_3031_);
v_snd_3032_ = lean_ctor_get(v_head_3029_, 1);
lean_inc(v_snd_3032_);
lean_dec(v_head_3029_);
v___x_3033_ = lean_apply_4(v_h__2_3027_, v_fst_3031_, v_snd_3032_, v_tail_3030_, v_x_3025_);
return v___x_3033_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_lookupCase_match__1_splitter(lean_object* v_motive_3034_, lean_object* v_x_3035_, lean_object* v_x_3036_, lean_object* v_h__1_3037_, lean_object* v_h__2_3038_){
_start:
{
if (lean_obj_tag(v_x_3035_) == 0)
{
lean_object* v___x_3039_; 
lean_dec(v_h__2_3038_);
v___x_3039_ = lean_apply_1(v_h__1_3037_, v_x_3036_);
return v___x_3039_;
}
else
{
lean_object* v_head_3040_; lean_object* v_tail_3041_; lean_object* v_fst_3042_; lean_object* v_snd_3043_; lean_object* v___x_3044_; 
lean_dec(v_h__1_3037_);
v_head_3040_ = lean_ctor_get(v_x_3035_, 0);
lean_inc(v_head_3040_);
v_tail_3041_ = lean_ctor_get(v_x_3035_, 1);
lean_inc(v_tail_3041_);
lean_dec_ref_known(v_x_3035_, 2);
v_fst_3042_ = lean_ctor_get(v_head_3040_, 0);
lean_inc(v_fst_3042_);
v_snd_3043_ = lean_ctor_get(v_head_3040_, 1);
lean_inc(v_snd_3043_);
lean_dec(v_head_3040_);
v___x_3044_ = lean_apply_4(v_h__2_3038_, v_fst_3042_, v_snd_3043_, v_tail_3041_, v_x_3036_);
return v___x_3044_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_unitLits_match__1_splitter___redArg(lean_object* v_x_3045_, lean_object* v_h__1_3046_, lean_object* v_h__2_3047_){
_start:
{
if (lean_obj_tag(v_x_3045_) == 1)
{
lean_object* v_tail_3048_; 
v_tail_3048_ = lean_ctor_get(v_x_3045_, 1);
if (lean_obj_tag(v_tail_3048_) == 0)
{
lean_object* v_head_3049_; lean_object* v___x_3050_; 
lean_dec(v_h__2_3047_);
v_head_3049_ = lean_ctor_get(v_x_3045_, 0);
lean_inc(v_head_3049_);
lean_dec_ref_known(v_x_3045_, 2);
v___x_3050_ = lean_apply_1(v_h__1_3046_, v_head_3049_);
return v___x_3050_;
}
else
{
lean_object* v___x_3051_; 
lean_dec(v_h__1_3046_);
v___x_3051_ = lean_apply_2(v_h__2_3047_, v_x_3045_, lean_box(0));
return v___x_3051_;
}
}
else
{
lean_object* v___x_3052_; 
lean_dec(v_h__1_3046_);
v___x_3052_ = lean_apply_2(v_h__2_3047_, v_x_3045_, lean_box(0));
return v___x_3052_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_unitLits_match__1_splitter(lean_object* v_motive_3053_, lean_object* v_x_3054_, lean_object* v_h__1_3055_, lean_object* v_h__2_3056_){
_start:
{
if (lean_obj_tag(v_x_3054_) == 1)
{
lean_object* v_tail_3057_; 
v_tail_3057_ = lean_ctor_get(v_x_3054_, 1);
if (lean_obj_tag(v_tail_3057_) == 0)
{
lean_object* v_head_3058_; lean_object* v___x_3059_; 
lean_dec(v_h__2_3056_);
v_head_3058_ = lean_ctor_get(v_x_3054_, 0);
lean_inc(v_head_3058_);
lean_dec_ref_known(v_x_3054_, 2);
v___x_3059_ = lean_apply_1(v_h__1_3055_, v_head_3058_);
return v___x_3059_;
}
else
{
lean_object* v___x_3060_; 
lean_dec(v_h__1_3055_);
v___x_3060_ = lean_apply_2(v_h__2_3056_, v_x_3054_, lean_box(0));
return v___x_3060_;
}
}
else
{
lean_object* v___x_3061_; 
lean_dec(v_h__1_3055_);
v___x_3061_ = lean_apply_2(v_h__2_3056_, v_x_3054_, lean_box(0));
return v___x_3061_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprConcl_repr_match__1_splitter___redArg(lean_object* v_x_3062_, lean_object* v_h__1_3063_, lean_object* v_h__2_3064_){
_start:
{
if (lean_obj_tag(v_x_3062_) == 0)
{
lean_object* v___x_3065_; lean_object* v___x_3066_; 
lean_dec(v_h__2_3064_);
v___x_3065_ = lean_box(0);
v___x_3066_ = lean_apply_1(v_h__1_3063_, v___x_3065_);
return v___x_3066_;
}
else
{
lean_object* v_hs_3067_; lean_object* v_ps_3068_; lean_object* v___x_3069_; 
lean_dec(v_h__1_3063_);
v_hs_3067_ = lean_ctor_get(v_x_3062_, 0);
lean_inc(v_hs_3067_);
v_ps_3068_ = lean_ctor_get(v_x_3062_, 1);
lean_inc(v_ps_3068_);
lean_dec_ref_known(v_x_3062_, 2);
v___x_3069_ = lean_apply_2(v_h__2_3064_, v_hs_3067_, v_ps_3068_);
return v___x_3069_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_instReprConcl_repr_match__1_splitter(lean_object* v_motive_3070_, lean_object* v_x_3071_, lean_object* v_h__1_3072_, lean_object* v_h__2_3073_){
_start:
{
if (lean_obj_tag(v_x_3071_) == 0)
{
lean_object* v___x_3074_; lean_object* v___x_3075_; 
lean_dec(v_h__2_3073_);
v___x_3074_ = lean_box(0);
v___x_3075_ = lean_apply_1(v_h__1_3072_, v___x_3074_);
return v___x_3075_;
}
else
{
lean_object* v_hs_3076_; lean_object* v_ps_3077_; lean_object* v___x_3078_; 
lean_dec(v_h__1_3072_);
v_hs_3076_ = lean_ctor_get(v_x_3071_, 0);
lean_inc(v_hs_3076_);
v_ps_3077_ = lean_ctor_get(v_x_3071_, 1);
lean_inc(v_ps_3077_);
lean_dec_ref_known(v_x_3071_, 2);
v___x_3078_ = lean_apply_2(v_h__2_3073_, v_hs_3076_, v_ps_3077_);
return v___x_3078_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tStmt_match__1_splitter___redArg(lean_object* v_x_3079_, lean_object* v_x_3080_, lean_object* v_h__1_3081_, lean_object* v_h__2_3082_, lean_object* v_h__3_3083_, lean_object* v_h__4_3084_, lean_object* v_h__5_3085_, lean_object* v_h__6_3086_){
_start:
{
switch(lean_obj_tag(v_x_3079_))
{
case 0:
{
lean_object* v_l_3087_; lean_object* v___x_3088_; 
lean_dec(v_h__6_3086_);
lean_dec(v_h__5_3085_);
lean_dec(v_h__4_3084_);
lean_dec(v_h__3_3083_);
lean_dec(v_h__2_3082_);
v_l_3087_ = lean_ctor_get(v_x_3079_, 0);
lean_inc(v_l_3087_);
lean_dec_ref_known(v_x_3079_, 1);
v___x_3088_ = lean_apply_2(v_h__1_3081_, v_l_3087_, v_x_3080_);
return v___x_3088_;
}
case 1:
{
lean_object* v_same_3089_; lean_object* v___x_3090_; 
lean_dec(v_h__6_3086_);
lean_dec(v_h__5_3085_);
lean_dec(v_h__4_3084_);
lean_dec(v_h__3_3083_);
lean_dec(v_h__1_3081_);
v_same_3089_ = lean_ctor_get(v_x_3079_, 0);
lean_inc(v_same_3089_);
lean_dec_ref_known(v_x_3079_, 1);
v___x_3090_ = lean_apply_2(v_h__2_3082_, v_same_3089_, v_x_3080_);
return v___x_3090_;
}
case 2:
{
lean_object* v_l_3091_; lean_object* v_d_3092_; lean_object* v___x_3093_; 
lean_dec(v_h__6_3086_);
lean_dec(v_h__5_3085_);
lean_dec(v_h__4_3084_);
lean_dec(v_h__2_3082_);
lean_dec(v_h__1_3081_);
v_l_3091_ = lean_ctor_get(v_x_3079_, 0);
lean_inc(v_l_3091_);
v_d_3092_ = lean_ctor_get(v_x_3079_, 1);
lean_inc(v_d_3092_);
lean_dec_ref_known(v_x_3079_, 2);
v___x_3093_ = lean_apply_3(v_h__3_3083_, v_l_3091_, v_d_3092_, v_x_3080_);
return v___x_3093_;
}
case 3:
{
lean_object* v_l_3094_; lean_object* v_m_3095_; lean_object* v___x_3096_; 
lean_dec(v_h__6_3086_);
lean_dec(v_h__5_3085_);
lean_dec(v_h__3_3083_);
lean_dec(v_h__2_3082_);
lean_dec(v_h__1_3081_);
v_l_3094_ = lean_ctor_get(v_x_3079_, 0);
lean_inc(v_l_3094_);
v_m_3095_ = lean_ctor_get(v_x_3079_, 1);
lean_inc(v_m_3095_);
lean_dec_ref_known(v_x_3079_, 2);
v___x_3096_ = lean_apply_3(v_h__4_3084_, v_l_3094_, v_m_3095_, v_x_3080_);
return v___x_3096_;
}
case 4:
{
lean_object* v_l_3097_; lean_object* v_m_3098_; lean_object* v_n_3099_; lean_object* v___x_3100_; 
lean_dec(v_h__6_3086_);
lean_dec(v_h__4_3084_);
lean_dec(v_h__3_3083_);
lean_dec(v_h__2_3082_);
lean_dec(v_h__1_3081_);
v_l_3097_ = lean_ctor_get(v_x_3079_, 0);
lean_inc(v_l_3097_);
v_m_3098_ = lean_ctor_get(v_x_3079_, 1);
lean_inc(v_m_3098_);
v_n_3099_ = lean_ctor_get(v_x_3079_, 2);
lean_inc(v_n_3099_);
lean_dec_ref_known(v_x_3079_, 3);
v___x_3100_ = lean_apply_4(v_h__5_3085_, v_l_3097_, v_m_3098_, v_n_3099_, v_x_3080_);
return v___x_3100_;
}
default: 
{
lean_object* v_l_3101_; lean_object* v_m_3102_; lean_object* v_n_3103_; lean_object* v___x_3104_; 
lean_dec(v_h__5_3085_);
lean_dec(v_h__4_3084_);
lean_dec(v_h__3_3083_);
lean_dec(v_h__2_3082_);
lean_dec(v_h__1_3081_);
v_l_3101_ = lean_ctor_get(v_x_3079_, 0);
lean_inc(v_l_3101_);
v_m_3102_ = lean_ctor_get(v_x_3079_, 1);
lean_inc(v_m_3102_);
v_n_3103_ = lean_ctor_get(v_x_3079_, 2);
lean_inc(v_n_3103_);
lean_dec_ref_known(v_x_3079_, 3);
v___x_3104_ = lean_apply_4(v_h__6_3086_, v_l_3101_, v_m_3102_, v_n_3103_, v_x_3080_);
return v___x_3104_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tStmt_match__1_splitter(lean_object* v_motive_3105_, lean_object* v_x_3106_, lean_object* v_x_3107_, lean_object* v_h__1_3108_, lean_object* v_h__2_3109_, lean_object* v_h__3_3110_, lean_object* v_h__4_3111_, lean_object* v_h__5_3112_, lean_object* v_h__6_3113_){
_start:
{
switch(lean_obj_tag(v_x_3106_))
{
case 0:
{
lean_object* v_l_3114_; lean_object* v___x_3115_; 
lean_dec(v_h__6_3113_);
lean_dec(v_h__5_3112_);
lean_dec(v_h__4_3111_);
lean_dec(v_h__3_3110_);
lean_dec(v_h__2_3109_);
v_l_3114_ = lean_ctor_get(v_x_3106_, 0);
lean_inc(v_l_3114_);
lean_dec_ref_known(v_x_3106_, 1);
v___x_3115_ = lean_apply_2(v_h__1_3108_, v_l_3114_, v_x_3107_);
return v___x_3115_;
}
case 1:
{
lean_object* v_same_3116_; lean_object* v___x_3117_; 
lean_dec(v_h__6_3113_);
lean_dec(v_h__5_3112_);
lean_dec(v_h__4_3111_);
lean_dec(v_h__3_3110_);
lean_dec(v_h__1_3108_);
v_same_3116_ = lean_ctor_get(v_x_3106_, 0);
lean_inc(v_same_3116_);
lean_dec_ref_known(v_x_3106_, 1);
v___x_3117_ = lean_apply_2(v_h__2_3109_, v_same_3116_, v_x_3107_);
return v___x_3117_;
}
case 2:
{
lean_object* v_l_3118_; lean_object* v_d_3119_; lean_object* v___x_3120_; 
lean_dec(v_h__6_3113_);
lean_dec(v_h__5_3112_);
lean_dec(v_h__4_3111_);
lean_dec(v_h__2_3109_);
lean_dec(v_h__1_3108_);
v_l_3118_ = lean_ctor_get(v_x_3106_, 0);
lean_inc(v_l_3118_);
v_d_3119_ = lean_ctor_get(v_x_3106_, 1);
lean_inc(v_d_3119_);
lean_dec_ref_known(v_x_3106_, 2);
v___x_3120_ = lean_apply_3(v_h__3_3110_, v_l_3118_, v_d_3119_, v_x_3107_);
return v___x_3120_;
}
case 3:
{
lean_object* v_l_3121_; lean_object* v_m_3122_; lean_object* v___x_3123_; 
lean_dec(v_h__6_3113_);
lean_dec(v_h__5_3112_);
lean_dec(v_h__3_3110_);
lean_dec(v_h__2_3109_);
lean_dec(v_h__1_3108_);
v_l_3121_ = lean_ctor_get(v_x_3106_, 0);
lean_inc(v_l_3121_);
v_m_3122_ = lean_ctor_get(v_x_3106_, 1);
lean_inc(v_m_3122_);
lean_dec_ref_known(v_x_3106_, 2);
v___x_3123_ = lean_apply_3(v_h__4_3111_, v_l_3121_, v_m_3122_, v_x_3107_);
return v___x_3123_;
}
case 4:
{
lean_object* v_l_3124_; lean_object* v_m_3125_; lean_object* v_n_3126_; lean_object* v___x_3127_; 
lean_dec(v_h__6_3113_);
lean_dec(v_h__4_3111_);
lean_dec(v_h__3_3110_);
lean_dec(v_h__2_3109_);
lean_dec(v_h__1_3108_);
v_l_3124_ = lean_ctor_get(v_x_3106_, 0);
lean_inc(v_l_3124_);
v_m_3125_ = lean_ctor_get(v_x_3106_, 1);
lean_inc(v_m_3125_);
v_n_3126_ = lean_ctor_get(v_x_3106_, 2);
lean_inc(v_n_3126_);
lean_dec_ref_known(v_x_3106_, 3);
v___x_3127_ = lean_apply_4(v_h__5_3112_, v_l_3124_, v_m_3125_, v_n_3126_, v_x_3107_);
return v___x_3127_;
}
default: 
{
lean_object* v_l_3128_; lean_object* v_m_3129_; lean_object* v_n_3130_; lean_object* v___x_3131_; 
lean_dec(v_h__5_3112_);
lean_dec(v_h__4_3111_);
lean_dec(v_h__3_3110_);
lean_dec(v_h__2_3109_);
lean_dec(v_h__1_3108_);
v_l_3128_ = lean_ctor_get(v_x_3106_, 0);
lean_inc(v_l_3128_);
v_m_3129_ = lean_ctor_get(v_x_3106_, 1);
lean_inc(v_m_3129_);
v_n_3130_ = lean_ctor_get(v_x_3106_, 2);
lean_inc(v_n_3130_);
lean_dec_ref_known(v_x_3106_, 3);
v___x_3131_ = lean_apply_4(v_h__6_3113_, v_l_3128_, v_m_3129_, v_n_3130_, v_x_3107_);
return v___x_3131_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_killAll_match__1_splitter___redArg(lean_object* v_x_3132_, lean_object* v_x_3133_, lean_object* v_h__1_3134_, lean_object* v_h__2_3135_){
_start:
{
if (lean_obj_tag(v_x_3132_) == 0)
{
lean_object* v___x_3136_; 
lean_dec(v_h__2_3135_);
v___x_3136_ = lean_apply_1(v_h__1_3134_, v_x_3133_);
return v___x_3136_;
}
else
{
lean_object* v_head_3137_; lean_object* v_tail_3138_; lean_object* v___x_3139_; 
lean_dec(v_h__1_3134_);
v_head_3137_ = lean_ctor_get(v_x_3132_, 0);
lean_inc(v_head_3137_);
v_tail_3138_ = lean_ctor_get(v_x_3132_, 1);
lean_inc(v_tail_3138_);
lean_dec_ref_known(v_x_3132_, 2);
v___x_3139_ = lean_apply_3(v_h__2_3135_, v_head_3137_, v_tail_3138_, v_x_3133_);
return v___x_3139_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_killAll_match__1_splitter(lean_object* v_motive_3140_, lean_object* v_x_3141_, lean_object* v_x_3142_, lean_object* v_h__1_3143_, lean_object* v_h__2_3144_){
_start:
{
if (lean_obj_tag(v_x_3141_) == 0)
{
lean_object* v___x_3145_; 
lean_dec(v_h__2_3144_);
v___x_3145_ = lean_apply_1(v_h__1_3143_, v_x_3142_);
return v___x_3145_;
}
else
{
lean_object* v_head_3146_; lean_object* v_tail_3147_; lean_object* v___x_3148_; 
lean_dec(v_h__1_3143_);
v_head_3146_ = lean_ctor_get(v_x_3141_, 0);
lean_inc(v_head_3146_);
v_tail_3147_ = lean_ctor_get(v_x_3141_, 1);
lean_inc(v_tail_3147_);
lean_dec_ref_known(v_x_3141_, 2);
v___x_3148_ = lean_apply_3(v_h__2_3144_, v_head_3146_, v_tail_3147_, v_x_3142_);
return v___x_3148_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_optEdge_match__1_splitter___redArg(lean_object* v_other_3149_, lean_object* v_h__1_3150_, lean_object* v_h__2_3151_){
_start:
{
if (lean_obj_tag(v_other_3149_) == 0)
{
lean_object* v___x_3152_; lean_object* v___x_3153_; 
lean_dec(v_h__1_3150_);
v___x_3152_ = lean_box(0);
v___x_3153_ = lean_apply_1(v_h__2_3151_, v___x_3152_);
return v___x_3153_;
}
else
{
lean_object* v_val_3154_; lean_object* v___x_3155_; 
lean_dec(v_h__2_3151_);
v_val_3154_ = lean_ctor_get(v_other_3149_, 0);
lean_inc(v_val_3154_);
lean_dec_ref_known(v_other_3149_, 1);
v___x_3155_ = lean_apply_1(v_h__1_3150_, v_val_3154_);
return v___x_3155_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_optEdge_match__1_splitter(lean_object* v_motive_3156_, lean_object* v_other_3157_, lean_object* v_h__1_3158_, lean_object* v_h__2_3159_){
_start:
{
if (lean_obj_tag(v_other_3157_) == 0)
{
lean_object* v___x_3160_; lean_object* v___x_3161_; 
lean_dec(v_h__1_3158_);
v___x_3160_ = lean_box(0);
v___x_3161_ = lean_apply_1(v_h__2_3159_, v___x_3160_);
return v___x_3161_;
}
else
{
lean_object* v_val_3162_; lean_object* v___x_3163_; 
lean_dec(v_h__2_3159_);
v_val_3162_ = lean_ctor_get(v_other_3157_, 0);
lean_inc(v_val_3162_);
lean_dec_ref_known(v_other_3157_, 1);
v___x_3163_ = lean_apply_1(v_h__1_3158_, v_val_3162_);
return v___x_3163_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tTerm_match__1_splitter___redArg(lean_object* v_x_3164_, lean_object* v_x_3165_, lean_object* v_h__1_3166_, lean_object* v_h__2_3167_, lean_object* v_h__3_3168_, lean_object* v_h__4_3169_, lean_object* v_h__5_3170_, lean_object* v_h__6_3171_){
_start:
{
switch(lean_obj_tag(v_x_3164_))
{
case 0:
{
lean_object* v_b_3172_; lean_object* v___x_3173_; 
lean_dec(v_h__6_3171_);
lean_dec(v_h__5_3170_);
lean_dec(v_h__4_3169_);
lean_dec(v_h__3_3168_);
lean_dec(v_h__2_3167_);
v_b_3172_ = lean_ctor_get(v_x_3164_, 0);
lean_inc(v_b_3172_);
lean_dec_ref_known(v_x_3164_, 1);
v___x_3173_ = lean_apply_2(v_h__1_3166_, v_b_3172_, v_x_3165_);
return v___x_3173_;
}
case 1:
{
lean_object* v_l_3174_; lean_object* v_cases_3175_; lean_object* v_other_3176_; lean_object* v___x_3177_; 
lean_dec(v_h__6_3171_);
lean_dec(v_h__5_3170_);
lean_dec(v_h__4_3169_);
lean_dec(v_h__3_3168_);
lean_dec(v_h__1_3166_);
v_l_3174_ = lean_ctor_get(v_x_3164_, 0);
lean_inc(v_l_3174_);
v_cases_3175_ = lean_ctor_get(v_x_3164_, 1);
lean_inc(v_cases_3175_);
v_other_3176_ = lean_ctor_get(v_x_3164_, 2);
lean_inc(v_other_3176_);
lean_dec_ref_known(v_x_3164_, 3);
v___x_3177_ = lean_apply_4(v_h__2_3167_, v_l_3174_, v_cases_3175_, v_other_3176_, v_x_3165_);
return v___x_3177_;
}
case 2:
{
lean_object* v_dsts_3178_; lean_object* v_b_3179_; lean_object* v_same_3180_; lean_object* v_post_3181_; lean_object* v___x_3182_; 
lean_dec(v_h__6_3171_);
lean_dec(v_h__5_3170_);
lean_dec(v_h__4_3169_);
lean_dec(v_h__2_3167_);
lean_dec(v_h__1_3166_);
v_dsts_3178_ = lean_ctor_get(v_x_3164_, 0);
lean_inc(v_dsts_3178_);
v_b_3179_ = lean_ctor_get(v_x_3164_, 1);
lean_inc(v_b_3179_);
v_same_3180_ = lean_ctor_get(v_x_3164_, 2);
lean_inc(v_same_3180_);
v_post_3181_ = lean_ctor_get(v_x_3164_, 3);
lean_inc(v_post_3181_);
lean_dec_ref_known(v_x_3164_, 4);
v___x_3182_ = lean_apply_5(v_h__3_3168_, v_dsts_3178_, v_b_3179_, v_same_3180_, v_post_3181_, v_x_3165_);
return v___x_3182_;
}
case 3:
{
lean_object* v_dst_3183_; lean_object* v_src_3184_; lean_object* v_pd_3185_; lean_object* v_ps_3186_; lean_object* v_b_3187_; lean_object* v___x_3188_; 
lean_dec(v_h__6_3171_);
lean_dec(v_h__5_3170_);
lean_dec(v_h__3_3168_);
lean_dec(v_h__2_3167_);
lean_dec(v_h__1_3166_);
v_dst_3183_ = lean_ctor_get(v_x_3164_, 0);
lean_inc(v_dst_3183_);
v_src_3184_ = lean_ctor_get(v_x_3164_, 1);
lean_inc(v_src_3184_);
v_pd_3185_ = lean_ctor_get(v_x_3164_, 2);
lean_inc(v_pd_3185_);
v_ps_3186_ = lean_ctor_get(v_x_3164_, 3);
lean_inc(v_ps_3186_);
v_b_3187_ = lean_ctor_get(v_x_3164_, 4);
lean_inc(v_b_3187_);
lean_dec_ref_known(v_x_3164_, 5);
v___x_3188_ = lean_apply_6(v_h__4_3169_, v_dst_3183_, v_src_3184_, v_pd_3185_, v_ps_3186_, v_b_3187_, v_x_3165_);
return v___x_3188_;
}
case 4:
{
lean_object* v___x_3189_; 
lean_dec(v_h__6_3171_);
lean_dec(v_h__4_3169_);
lean_dec(v_h__3_3168_);
lean_dec(v_h__2_3167_);
lean_dec(v_h__1_3166_);
v___x_3189_ = lean_apply_1(v_h__5_3170_, v_x_3165_);
return v___x_3189_;
}
default: 
{
lean_object* v___x_3190_; 
lean_dec(v_h__5_3170_);
lean_dec(v_h__4_3169_);
lean_dec(v_h__3_3168_);
lean_dec(v_h__2_3167_);
lean_dec(v_h__1_3166_);
v___x_3190_ = lean_apply_1(v_h__6_3171_, v_x_3165_);
return v___x_3190_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_tTerm_match__1_splitter(lean_object* v_motive_3191_, lean_object* v_x_3192_, lean_object* v_x_3193_, lean_object* v_h__1_3194_, lean_object* v_h__2_3195_, lean_object* v_h__3_3196_, lean_object* v_h__4_3197_, lean_object* v_h__5_3198_, lean_object* v_h__6_3199_){
_start:
{
switch(lean_obj_tag(v_x_3192_))
{
case 0:
{
lean_object* v_b_3200_; lean_object* v___x_3201_; 
lean_dec(v_h__6_3199_);
lean_dec(v_h__5_3198_);
lean_dec(v_h__4_3197_);
lean_dec(v_h__3_3196_);
lean_dec(v_h__2_3195_);
v_b_3200_ = lean_ctor_get(v_x_3192_, 0);
lean_inc(v_b_3200_);
lean_dec_ref_known(v_x_3192_, 1);
v___x_3201_ = lean_apply_2(v_h__1_3194_, v_b_3200_, v_x_3193_);
return v___x_3201_;
}
case 1:
{
lean_object* v_l_3202_; lean_object* v_cases_3203_; lean_object* v_other_3204_; lean_object* v___x_3205_; 
lean_dec(v_h__6_3199_);
lean_dec(v_h__5_3198_);
lean_dec(v_h__4_3197_);
lean_dec(v_h__3_3196_);
lean_dec(v_h__1_3194_);
v_l_3202_ = lean_ctor_get(v_x_3192_, 0);
lean_inc(v_l_3202_);
v_cases_3203_ = lean_ctor_get(v_x_3192_, 1);
lean_inc(v_cases_3203_);
v_other_3204_ = lean_ctor_get(v_x_3192_, 2);
lean_inc(v_other_3204_);
lean_dec_ref_known(v_x_3192_, 3);
v___x_3205_ = lean_apply_4(v_h__2_3195_, v_l_3202_, v_cases_3203_, v_other_3204_, v_x_3193_);
return v___x_3205_;
}
case 2:
{
lean_object* v_dsts_3206_; lean_object* v_b_3207_; lean_object* v_same_3208_; lean_object* v_post_3209_; lean_object* v___x_3210_; 
lean_dec(v_h__6_3199_);
lean_dec(v_h__5_3198_);
lean_dec(v_h__4_3197_);
lean_dec(v_h__2_3195_);
lean_dec(v_h__1_3194_);
v_dsts_3206_ = lean_ctor_get(v_x_3192_, 0);
lean_inc(v_dsts_3206_);
v_b_3207_ = lean_ctor_get(v_x_3192_, 1);
lean_inc(v_b_3207_);
v_same_3208_ = lean_ctor_get(v_x_3192_, 2);
lean_inc(v_same_3208_);
v_post_3209_ = lean_ctor_get(v_x_3192_, 3);
lean_inc(v_post_3209_);
lean_dec_ref_known(v_x_3192_, 4);
v___x_3210_ = lean_apply_5(v_h__3_3196_, v_dsts_3206_, v_b_3207_, v_same_3208_, v_post_3209_, v_x_3193_);
return v___x_3210_;
}
case 3:
{
lean_object* v_dst_3211_; lean_object* v_src_3212_; lean_object* v_pd_3213_; lean_object* v_ps_3214_; lean_object* v_b_3215_; lean_object* v___x_3216_; 
lean_dec(v_h__6_3199_);
lean_dec(v_h__5_3198_);
lean_dec(v_h__3_3196_);
lean_dec(v_h__2_3195_);
lean_dec(v_h__1_3194_);
v_dst_3211_ = lean_ctor_get(v_x_3192_, 0);
lean_inc(v_dst_3211_);
v_src_3212_ = lean_ctor_get(v_x_3192_, 1);
lean_inc(v_src_3212_);
v_pd_3213_ = lean_ctor_get(v_x_3192_, 2);
lean_inc(v_pd_3213_);
v_ps_3214_ = lean_ctor_get(v_x_3192_, 3);
lean_inc(v_ps_3214_);
v_b_3215_ = lean_ctor_get(v_x_3192_, 4);
lean_inc(v_b_3215_);
lean_dec_ref_known(v_x_3192_, 5);
v___x_3216_ = lean_apply_6(v_h__4_3197_, v_dst_3211_, v_src_3212_, v_pd_3213_, v_ps_3214_, v_b_3215_, v_x_3193_);
return v___x_3216_;
}
case 4:
{
lean_object* v___x_3217_; 
lean_dec(v_h__6_3199_);
lean_dec(v_h__4_3197_);
lean_dec(v_h__3_3196_);
lean_dec(v_h__2_3195_);
lean_dec(v_h__1_3194_);
v___x_3217_ = lean_apply_1(v_h__5_3198_, v_x_3193_);
return v___x_3217_;
}
default: 
{
lean_object* v___x_3218_; 
lean_dec(v_h__5_3198_);
lean_dec(v_h__4_3197_);
lean_dec(v_h__3_3196_);
lean_dec(v_h__2_3195_);
lean_dec(v_h__1_3194_);
v___x_3218_ = lean_apply_1(v_h__6_3199_, v_x_3193_);
return v___x_3218_;
}
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_checkCert_match__1_splitter___redArg(lean_object* v_x_3219_, lean_object* v_h__1_3220_, lean_object* v_h__2_3221_){
_start:
{
if (lean_obj_tag(v_x_3219_) == 1)
{
lean_object* v_val_3222_; 
v_val_3222_ = lean_ctor_get(v_x_3219_, 0);
if (lean_obj_tag(v_val_3222_) == 1)
{
lean_object* v_val_3223_; lean_object* v___x_3224_; 
lean_inc_ref(v_val_3222_);
lean_dec_ref_known(v_x_3219_, 1);
lean_dec(v_h__2_3221_);
v_val_3223_ = lean_ctor_get(v_val_3222_, 0);
lean_inc(v_val_3223_);
lean_dec_ref_known(v_val_3222_, 1);
v___x_3224_ = lean_apply_1(v_h__1_3220_, v_val_3223_);
return v___x_3224_;
}
else
{
lean_object* v___x_3225_; 
lean_dec(v_h__1_3220_);
v___x_3225_ = lean_apply_2(v_h__2_3221_, v_x_3219_, lean_box(0));
return v___x_3225_;
}
}
else
{
lean_object* v___x_3226_; 
lean_dec(v_h__1_3220_);
v___x_3226_ = lean_apply_2(v_h__2_3221_, v_x_3219_, lean_box(0));
return v___x_3226_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_checkCert_match__1_splitter(lean_object* v_motive_3227_, lean_object* v_x_3228_, lean_object* v_h__1_3229_, lean_object* v_h__2_3230_){
_start:
{
if (lean_obj_tag(v_x_3228_) == 1)
{
lean_object* v_val_3231_; 
v_val_3231_ = lean_ctor_get(v_x_3228_, 0);
if (lean_obj_tag(v_val_3231_) == 1)
{
lean_object* v_val_3232_; lean_object* v___x_3233_; 
lean_inc_ref(v_val_3231_);
lean_dec_ref_known(v_x_3228_, 1);
lean_dec(v_h__2_3230_);
v_val_3232_ = lean_ctor_get(v_val_3231_, 0);
lean_inc(v_val_3232_);
lean_dec_ref_known(v_val_3231_, 1);
v___x_3233_ = lean_apply_1(v_h__1_3229_, v_val_3232_);
return v___x_3233_;
}
else
{
lean_object* v___x_3234_; 
lean_dec(v_h__1_3229_);
v___x_3234_ = lean_apply_2(v_h__2_3230_, v_x_3228_, lean_box(0));
return v___x_3234_;
}
}
else
{
lean_object* v___x_3235_; 
lean_dec(v_h__1_3229_);
v___x_3235_ = lean_apply_2(v_h__2_3230_, v_x_3228_, lean_box(0));
return v___x_3235_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_edgeOk_match__1_splitter___redArg(lean_object* v_x_3236_, lean_object* v_h__1_3237_, lean_object* v_h__2_3238_){
_start:
{
if (lean_obj_tag(v_x_3236_) == 1)
{
lean_object* v_val_3239_; 
v_val_3239_ = lean_ctor_get(v_x_3236_, 0);
if (lean_obj_tag(v_val_3239_) == 1)
{
lean_object* v_val_3240_; lean_object* v___x_3241_; 
lean_inc_ref(v_val_3239_);
lean_dec_ref_known(v_x_3236_, 1);
lean_dec(v_h__2_3238_);
v_val_3240_ = lean_ctor_get(v_val_3239_, 0);
lean_inc(v_val_3240_);
lean_dec_ref_known(v_val_3239_, 1);
v___x_3241_ = lean_apply_1(v_h__1_3237_, v_val_3240_);
return v___x_3241_;
}
else
{
lean_object* v___x_3242_; 
lean_dec(v_h__1_3237_);
v___x_3242_ = lean_apply_2(v_h__2_3238_, v_x_3236_, lean_box(0));
return v___x_3242_;
}
}
else
{
lean_object* v___x_3243_; 
lean_dec(v_h__1_3237_);
v___x_3243_ = lean_apply_2(v_h__2_3238_, v_x_3236_, lean_box(0));
return v___x_3243_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_edgeOk_match__1_splitter(lean_object* v_motive_3244_, lean_object* v_x_3245_, lean_object* v_h__1_3246_, lean_object* v_h__2_3247_){
_start:
{
if (lean_obj_tag(v_x_3245_) == 1)
{
lean_object* v_val_3248_; 
v_val_3248_ = lean_ctor_get(v_x_3245_, 0);
if (lean_obj_tag(v_val_3248_) == 1)
{
lean_object* v_val_3249_; lean_object* v___x_3250_; 
lean_inc_ref(v_val_3248_);
lean_dec_ref_known(v_x_3245_, 1);
lean_dec(v_h__2_3247_);
v_val_3249_ = lean_ctor_get(v_val_3248_, 0);
lean_inc(v_val_3249_);
lean_dec_ref_known(v_val_3248_, 1);
v___x_3250_ = lean_apply_1(v_h__1_3246_, v_val_3249_);
return v___x_3250_;
}
else
{
lean_object* v___x_3251_; 
lean_dec(v_h__1_3246_);
v___x_3251_ = lean_apply_2(v_h__2_3247_, v_x_3245_, lean_box(0));
return v___x_3251_;
}
}
else
{
lean_object* v___x_3252_; 
lean_dec(v_h__1_3246_);
v___x_3252_ = lean_apply_2(v_h__2_3247_, v_x_3245_, lean_box(0));
return v___x_3252_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_blockOk_match__1_splitter___redArg(lean_object* v_x_3253_, lean_object* v_h__1_3254_, lean_object* v_h__2_3255_){
_start:
{
if (lean_obj_tag(v_x_3253_) == 4)
{
lean_object* v___x_3256_; lean_object* v___x_3257_; 
lean_dec(v_h__2_3255_);
v___x_3256_ = lean_box(0);
v___x_3257_ = lean_apply_1(v_h__1_3254_, v___x_3256_);
return v___x_3257_;
}
else
{
lean_object* v___x_3258_; 
lean_dec(v_h__1_3254_);
v___x_3258_ = lean_apply_2(v_h__2_3255_, v_x_3253_, lean_box(0));
return v___x_3258_;
}
}
}
LEAN_EXPORT lean_object* lp_clauseflow___private_ClauseFlow_Core_0__ClauseFlow_blockOk_match__1_splitter(lean_object* v_motive_3259_, lean_object* v_x_3260_, lean_object* v_h__1_3261_, lean_object* v_h__2_3262_){
_start:
{
if (lean_obj_tag(v_x_3260_) == 4)
{
lean_object* v___x_3263_; lean_object* v___x_3264_; 
lean_dec(v_h__2_3262_);
v___x_3263_ = lean_box(0);
v___x_3264_ = lean_apply_1(v_h__1_3261_, v___x_3263_);
return v___x_3264_;
}
else
{
lean_object* v___x_3265_; 
lean_dec(v_h__1_3261_);
v___x_3265_ = lean_apply_2(v_h__2_3262_, v_x_3260_, lean_box(0));
return v___x_3265_;
}
}
}
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_Init(uint8_t builtin);
void lean_initialize_runtime_module();
static bool _G_initialized = false;
LEAN_EXPORT lean_object* initialize_clauseflow_ClauseFlow_Core(uint8_t builtin) {
lean_object * res;
if (_G_initialized) return lean_io_result_mk_ok(lean_box(0));
_G_initialized = true;
lean_initialize_runtime_module();
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
return lean_io_result_mk_ok(lean_box(0));
}
#ifdef __cplusplus
}
#endif
