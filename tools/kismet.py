"""Summarise Blueprint (Kismet) bytecode exported by tools/rmx (run with --script) into compact,
readable statements, one text file per asset, for studying gameplay rules.

    python tools/kismet.py tools/rmx/out tools/rmx/out_bp

Each output file lists the class defaults and every function as numbered statements, e.g.
    [12] Result = SelectFloat(1.4, 1.0, GetTargetDataNamed(TargetData.IsSprinting, ...))
The output is derived from the game's code: keep it local (out_bp/ is git-ignored), and write
rules found in it in your own words.
"""

import json
import sys
from pathlib import Path


def name_of(ref):
    """Short name of an object/function reference like "Class'KismetMathLibrary:SelectFloat'"."""
    if isinstance(ref, dict):
        ref = ref.get("ObjectName") or ref.get("Name") or ""
    ref = str(ref)
    if "'" in ref:
        ref = ref.split("'")[1]
    return ref.split(":")[-1].split(".")[-1]


def var_name(v):
    if not isinstance(v, dict):
        return str(v)
    if "Inst" in v:
        return expr(v)
    prop = v.get("Property")
    if isinstance(prop, dict) and prop.get("Name"):
        return prop["Name"]
    if v.get("Path"):
        return ".".join(map(str, v["Path"]))
    new = v.get("New")
    if isinstance(new, dict) and new.get("Path"):
        return ".".join(map(str, new["Path"]))
    return "?"


def args(params):
    return ", ".join(expr(p) for p in params or [])


CONST = {"EX_IntConst", "EX_Int64Const", "EX_UInt64Const", "EX_ByteConst", "EX_IntConstByte", "EX_NameConst", "EX_SkipOffsetConst"}


def expr(e):
    if not isinstance(e, dict):
        return str(e)
    i = e.get("Inst", "")
    if i in CONST:
        return str(e.get("Value"))
    if i in ("EX_FloatConst", "EX_DoubleConst"):
        v = e.get("Value")
        return f"{v:g}" if isinstance(v, (int, float)) else str(v)
    if i in ("EX_StringConst", "EX_UnicodeStringConst"):
        return json.dumps(e.get("Value"))
    if i == "EX_TextConst":
        v = e.get("Value", {})
        return json.dumps(v.get("SourceString") or v.get("KeyString") or "text")
    if i == "EX_True":
        return "true"
    if i == "EX_False":
        return "false"
    if i in ("EX_NoObject", "EX_NoInterface"):
        return "null"
    if i == "EX_Self":
        return "self"
    if i in ("EX_ObjectConst", "EX_SoftObjectConst", "EX_PropertyConst", "EX_FieldPathConst"):
        return name_of(e.get("Value") or e.get("Property"))
    if i in ("EX_VectorConst", "EX_Vector3fConst", "EX_RotationConst"):
        v = e.get("Value", {})
        return "(" + ", ".join(f"{v.get(k, 0):g}" for k in ("X", "Y", "Z", "Pitch", "Yaw", "Roll") if k in v) + ")"
    if i == "EX_TransformConst":
        return "Transform(...)"
    if i == "EX_StructConst":
        return f"{name_of(e.get('Struct'))}{{{args(e.get('Properties'))}}}"
    if i in ("EX_ArrayConst", "EX_SetConst"):
        return "[" + args(e.get("Elements")) + "]"
    if i == "EX_MapConst":
        return "{" + args(e.get("Elements")) + "}"
    if i in ("EX_LocalVariable", "EX_InstanceVariable", "EX_DefaultVariable", "EX_LocalOutVariable", "EX_ClassSparseDataVariable"):
        prefix = {"EX_InstanceVariable": "this.", "EX_DefaultVariable": "defaults."}.get(i, "")
        return prefix + var_name(e.get("Variable"))
    if i.startswith("EX_Let"):
        target = expr(e["Variable"]) if isinstance(e.get("Variable"), dict) and "Inst" in e["Variable"] else var_name(e.get("Variable"))
        return f"{target} = {expr(e.get('Expression') or e.get('Value') or e.get('Assignment'))}"
    if i in ("EX_FinalFunction", "EX_LocalFinalFunction", "EX_CallMath", "EX_VirtualFunction", "EX_LocalVirtualFunction"):
        fn = e.get("Function") or e.get("VirtualFunctionName")
        return f"{name_of(fn)}({args(e.get('Parameters'))})"
    if i == "EX_CallMulticastDelegate":
        return f"broadcast {name_of(e.get('StackNode'))}({args(e.get('Parameters'))})"
    if i in ("EX_Context", "EX_Context_FailSilent", "EX_ClassContext"):
        return f"{expr(e.get('ObjectExpression'))}.{expr(e.get('ContextExpression'))}"
    if i == "EX_InterfaceContext":
        return expr(e.get("InterfaceValue"))
    if i == "EX_StructMemberContext":
        prop = e.get("Property", {})
        member = ".".join(prop.get("Path", [])) if isinstance(prop, dict) else str(prop)
        return f"{expr(e.get('StructExpression'))}.{member}"
    if i in ("EX_DynamicCast", "EX_MetaCast", "EX_ObjToInterfaceCast", "EX_CrossInterfaceCast", "EX_InterfaceToObjCast"):
        return f"Cast<{name_of(e.get('ClassPtr') or e.get('InterfaceClass'))}>({expr(e.get('Target'))})"
    if i == "EX_PrimitiveCast":
        return f"{e.get('ConversionType', 'cast')}({expr(e.get('Target'))})"
    if i == "EX_ArrayGetByRef":
        return f"{expr(e.get('ArrayVariable'))}[{expr(e.get('ArrayIndex'))}]"
    if i in ("EX_SetArray", "EX_SetSet", "EX_SetMap"):
        target = e.get("AssigningProperty") or e.get("SetProperty") or e.get("MapProperty")
        return f"{expr(target)} = [{args(e.get('Elements'))}]"
    if i == "EX_SwitchValue":
        cases = "; ".join(f"{expr(c.get('CaseIndexValueTerm'))}: {expr(c.get('CaseTerm'))}" for c in e.get("Cases", []))
        return f"switch({expr(e.get('IndexTerm'))}) {{{cases}; default: {expr(e.get('DefaultTerm'))}}}"
    if i == "EX_BindDelegate":
        return f"bind {e.get('FunctionName')} on {expr(e.get('ObjectTerm'))}"
    if i in ("EX_AddMulticastDelegate", "EX_RemoveMulticastDelegate", "EX_ClearMulticastDelegate"):
        return f"{i[3:]}({expr(e.get('Delegate'))}, {expr(e.get('DelegateToAdd'))})"
    if i == "EX_InstanceDelegate":
        return f"delegate {e.get('FunctionName')}"
    # Flow
    if i == "EX_Jump":
        return f"goto {e.get('CodeOffset')}"
    if i == "EX_JumpIfNot":
        return f"if not ({expr(e.get('BooleanExpression'))}) goto {e.get('CodeOffset')}"
    if i == "EX_ComputedJump":
        return f"goto {expr(e.get('CodeOffsetExpression'))}"
    if i == "EX_PushExecutionFlow":
        return f"push flow {e.get('PushingAddress')}"
    if i == "EX_PopExecutionFlow":
        return "pop flow"
    if i == "EX_PopExecutionFlowIfNot":
        return f"if not ({expr(e.get('BooleanExpression'))}) pop flow"
    if i == "EX_Return":
        return f"return {expr(e.get('ReturnExpression'))}"
    if i in ("EX_Nothing", "EX_EndOfScript", "EX_Tracepoint", "EX_WireTracepoint", "EX_InstrumentationEvent", "EX_NothingInt32"):
        return ""
    # Fallback: name plus any nested expressions.
    kids = [expr(v) for k, v in e.items() if isinstance(v, dict) and "Inst" in v]
    kids += [expr(x) for k, v in e.items() if isinstance(v, list) for x in v if isinstance(x, dict) and "Inst" in x]
    return f"{i[3:]}({', '.join(kids)})"


def short(v, limit=300):
    s = json.dumps(v, ensure_ascii=False)
    return s if len(s) <= limit else s[:limit] + "..."


def summarise(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    lines = []
    for o in data:
        name, typ = o.get("Name", ""), o.get("Type", "")
        props = o.get("Properties") or {}
        if name.startswith("Default__") and props:
            lines.append(f"## defaults {name}")
            for k, v in props.items():
                if k != "UberGraphFrame":
                    lines.append(f"  {k} = {short(v)}")
        bc = o.get("ScriptBytecode")
        if bc:
            lines.append(f"## function {typ} {name}")
            for st in bc:
                text = expr(st)
                if text:
                    idx = st.get("StatementIndex")
                    lines.append(f"  [{idx}] {text}" if idx is not None else f"  {text}")
    return lines


def main(src, dst):
    src, dst = Path(src), Path(dst)
    dst.mkdir(parents=True, exist_ok=True)
    index = []
    for f in sorted(src.glob("*.json")):
        try:
            lines = summarise(f)
        except Exception as e:  # keep going; note the failure in the index
            index.append(f"{f.stem}\tERROR {e}")
            continue
        if not lines:
            continue
        functions = sum(1 for line in lines if line.startswith("## function"))
        (dst / f"{f.stem}.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
        index.append(f"{f.stem}\t{functions} functions\t{len(lines)} lines")
    (dst / "_index.tsv").write_text("\n".join(index) + "\n", encoding="utf-8")
    print(f"{len(index)} assets summarised into {dst}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
