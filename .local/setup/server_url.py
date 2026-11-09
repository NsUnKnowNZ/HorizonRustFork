import json, os
p = os.path.join(os.environ["APPDATA"], ".minecraft", "versions", "26.3", "26.3.json")
s = json.load(open(p))["downloads"]["server"]
print("SERVER_URL", s["url"], s.get("size"), s.get("sha1"))
