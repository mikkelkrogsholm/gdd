"""Concrete static-import checker for the fixed parcel package.

Exit 0 means PASS, 1 means a known violation, and 2 means UNKNOWN.
Dynamic import syntax or missing/parsing-failed source remains UNKNOWN.
"""
import ast
import json
from pathlib import Path
import sys


def check(source):
    source = Path(source)
    edges=set()
    for module in ("cli","pricing","tariffs"):
        path=source/"parcel"/(module+".py")
        if not path.is_file():
            print(json.dumps({"result":"UNKNOWN","reason":"missing actual code","resource":str(path)}))
            return 2
        try:tree=ast.parse(path.read_text(),filename=str(path))
        except (SyntaxError,OSError) as exc:
            print(json.dumps({"result":"UNKNOWN","reason":str(exc)}));return 2
        for node in ast.walk(tree):
            if isinstance(node,ast.Call) and ((isinstance(node.func,ast.Name) and node.func.id=="__import__") or (isinstance(node.func,ast.Attribute) and node.func.attr=="import_module")):
                print(json.dumps({"result":"UNKNOWN","reason":"dynamic import outside static checker profile"}));return 2
            targets=[]
            if isinstance(node,ast.Import):targets=[x.name for x in node.names]
            elif isinstance(node,ast.ImportFrom):
                if node.level == 1:targets=[node.module] if node.module else [x.name for x in node.names]
                elif node.module == "parcel":targets=[x.name for x in node.names]
                elif node.module:targets=[node.module]
            for target in targets:
                if target.startswith("parcel."):target=target.removeprefix("parcel.").split(".")[0]
                if target in {"cli","pricing","tariffs"}:edges.add((module,target))
    forbidden=("cli","tariffs") in edges
    missing={('cli','pricing'),('pricing','tariffs')}-edges
    result="FAIL" if forbidden or missing else "PASS"
    print(json.dumps({"result":result,"actual_imports":sorted(edges),"forbidden_cli_tariffs":forbidden,"missing_required_imports":sorted(missing)}))
    return 1 if result=="FAIL" else 0


if __name__ == "__main__":
    raise SystemExit(check(sys.argv[1] if len(sys.argv)>1 else "src"))
