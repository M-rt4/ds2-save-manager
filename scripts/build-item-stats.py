"""Builds src/public/item_stats.json: what the game's item screen shows for every item in
src-tauri/data/items.json, read from an installed copy of Dark Souls II.

Sources (archive and regulation reading live in ds2data.py):
- The regulation's params, laid out by soulsmods/Paramdex (DS2S/Defs). ItemParam links an item
  to its WeaponParam / ArmorParam / RingParam / SpellParam / ArrowParam row.
- Weapons and shields: WeaponReinforceParam holds each damage type at +0 and at the highest
  level, and the share ("Damage %") the weapon deals of each; the game interpolates linearly and
  rounds down. Poison and bleed show a fifth of their value. WeaponStatsAffectParam holds the
  scaling per level, FeWeaponThreshold the menu's letter thresholds (row 1 Strength, row 2
  Dexterity, rows 3-6 the elements, which share one set). WeaponTypeParam byte 0x36 is the
  weapon type's text id in weapontype.fmg.
- Armour: ArmorReinforceParam at +0 and at the highest level; elemental defence and
  resistances are stored as a tenth of what the menu shows.
- Text: ItemName / SimpleExplanation / DetailedExplanation FMGs, keyed by item id.

Checked against the game's own numbers (Dagger, Broadsword, Bandit's Knife, Sorcerer's Staff,
Moonlight Greatsword, Pate's Helm, Soul Arrow): every level of their upgrade tables matches.
Infusions change a weapon's numbers through other params; those are not modelled.

usage: python scripts/build-item-stats.py "<game folder>" "<Paramdex>/DS2S/Defs"
needs: pip install cryptography
"""

import json
import os
import struct
import sys

from ds2data import Archive, Param, ParamDef, fmg, regulation

OUT = "src/public/item_stats.json"
LETTERS = "SABCD"
# WeaponStatsAffectParam: two header floats, then per level STR, DEX, Magic, Lightning, Fire,
# Dark, Poison, Bleed and one unknown.
AFFECT_PER_LEVEL = 9
# Damage types in the order the menu lists them, with their WeaponStatsAffect column.
ELEMENTS = [("Magic", 2), ("Fire", 4), ("Lightning", 3), ("Dark", 5)]
DAMAGE = ["Physical", "Magic", "Fire", "Lightning", "Dark"]
ATTACK_KINDS = {1: "strike", 2: "slash", 3: "thrust"}
SPELL_ATTUNEMENT = [10, 15, 26, 32, 38, 43, 49, 58, 79, 94]
SPELL_CASTS = ["Base Casts"] + [f"Casts ({a} ATN)" for a in SPELL_ATTUNEMENT[1:]]


def num(v, places=2):
    """A float rounded for the JSON, as an int when it is one."""
    r = round(v, places)
    return int(r) if r == int(r) else r


def letter(value, thresholds):
    for mark, limit in zip(LETTERS, thresholds):
        if value >= limit:
            return mark
    return "E"


class Game:
    def __init__(self, game, defs):
        self.reg = regulation(game)

        def param(file, xml):
            return Param(self.reg[file], ParamDef(os.path.join(defs, xml)))

        self.item = param("ItemParam.param", "ITEM_PARAM.xml")
        self.weapon = param("WeaponParam.param", "WEAPON_PARAM.xml")
        self.reinforce = param("WeaponReinforceParam.param", "WEAPON_REINFORCE_PARAM.xml")
        self.affect = param("WeaponStatsAffectParam.param", "WEAPON_STATS_AFFECT_PARAM.xml")
        self.armor = param("ArmorParam.param", "ARMOR_PARAM.xml")
        self.armor_reinforce = param("ArmorReinforceParam.param", "ARMOR_REINFORCE_PARAM.xml")
        self.ring = param("RingParam.param", "RING_PARAM.xml")
        self.spell = param("SpellParam.param", "SPELL_PARAM.xml")
        self.arrow = param("ArrowParam.param", "ARROW_PARAM.xml")

        data = self.reg["FeWeaponThreshold.param"]
        self.thresholds = {rid: struct.unpack_from("<5f", data, off) for rid, off in Param(data, _Empty()).rows.items()}
        types = self.reg["WeaponTypeParam.param"]
        self.type_text = {rid: types[off + 0x36] for rid, off in Param(types, _Empty()).rows.items()}

        archive = Archive(game)
        text = lambda name: fmg(archive.read_path(f"/menu/text/english/{name}.fmg"))  # noqa: E731
        self.names = text("itemname")
        self.simple = text("simpleexplanation")
        self.detail = text("detailedexplanation")
        self.type_names = text("weapontype")

    def scaling(self, affect_id, level, shares):
        """The menu's six bonus letters at a level: STR, DEX, Magic, Fire, Lightning, Dark."""
        row = self.affect.get(affect_id)
        if row is None:
            return "------"
        values = list(row.values())[2 + level * AFFECT_PER_LEVEL :][:AFFECT_PER_LEVEL]
        out = []
        for column, physical_row in ((0, 1), (1, 2)):
            v = values[column]
            out.append(letter(v, self.thresholds[physical_row]) if shares["Physical"] > 0 and v > 0 else "-")
        for name, column in ELEMENTS:
            v = values[column]
            out.append(letter(v, self.thresholds[3]) if shares[name] > 0 and v > 0 else "-")
        return "".join(out)

    def weapon_stats(self, wid):
        w = self.weapon.get(wid)
        r = w and self.reinforce.get(w["Weapon Reinforce ID"])
        if not r:
            return None
        shares = {k: r[f"Damage %: {k}"] / 100 for k in DAMAGE + ["Poison", "Bleed"]}
        level = max(0, r["Max Reinforcement Level"])
        kind = w["Damage Type"]
        type_id = self.type_text.get(w["Weapon Type ID"])
        stability = [num(r[f"Stability: +{n}"]) for n in range(min(level, 10) + 1)]
        return {
            "type": self.type_names.get(type_id) if type_id is not None else None,
            "hit": [ATTACK_KINDS[k] for k in (kind & 0xFF, kind >> 8 & 0xFF) if k in ATTACK_KINDS],
            "req": [max(0, w[f"{s} Requirement"]) for s in ("STR", "DEX", "INT", "FTH")],
            "lv": level,
            "dmg": [
                [num(r[f"Damage: {k}"] * shares[k]) for k in DAMAGE],
                [num(r[f"Max Damage: {k}"] * shares[k]) for k in DAMAGE],
            ],
            "aux": [
                [num(r[f"Damage: {k} (Float / 5)"] * shares[k]) for k in ("Poison", "Bleed")],
                [num(r[f"Max Damage: {k} (Float / 5)"] * shares[k]) for k in ("Poison", "Bleed")],
            ],
            "sc": [self.scaling(r["Weapon Stats Affect ID"], n, shares) for n in range(level + 1)],
            "abs": [num(r[f"Absorption: {k}"], 1) for k in DAMAGE],
            "res": [num(r[f"Resistance: {k}"], 1) for k in ("Poison", "Bleed", "Petrify", "Curse")],
            "stab": stability[0] if len(set(stability)) == 1 else stability,
            "counter": max(0, w["Counter Damage"]),
            "poise": max(0, w["Poise Damage"]),
            "cast": max(0, w["Casting Speed"]),
            "wt": num(w["Weight"]),
            "dur": num(w["Max Durability"]),
        }

    def armor_stats(self, aid):
        a = self.armor.get(aid)
        r = a and self.armor_reinforce.get(a["Armor Reinforce ID"])
        if not r:
            return None
        physical = ["Standard Defence", "Strike Defence", "Slash Defence", "Thrust Defence"]
        tenths = ["Magic Absorption", "Fire Absorption", "Lightning Absorption", "Dark Absorption",
                  "Poison Resist", "Bleed Resist", "Petrify Resist", "Curse Resist"]
        top = {"Strike Defence": "Make Strike Defence"}  # sic, as Paramdex names it

        def at(field, scale):
            return num(r[field] * scale, 3), num(r[top.get(field, "Max " + field)] * scale, 3)

        pairs = [at(f, 1) for f in physical] + [at(f, 10) for f in tenths]
        return {
            "slot": a["Slot Category"],
            "req": [a[f"{s} Requirement"] for s in ("STR", "DEX", "INT", "FAI")],
            "lv": max(0, r["Max Reinforce Level"]),
            "def": [[p[0] for p in pairs], [p[1] for p in pairs]],
            "poise": num(a["Poise"], 1),
            "wt": num(a["Weight"]),
            "dur": num(a["Durability"]),
        }

    def spell_stats(self, sid):
        s = self.spell.get(sid)
        if not s:
            return None
        return {
            "req": [s["INT Requirement"], s["FTH Requirement"]],
            "slots": s["Slots Used"],
            "casts": [s[c] for c in SPELL_CASTS],
        }

    def arrow_stats(self, aid):
        a = self.arrow.get(aid)
        if not a:
            return None
        return {"dmg": [a[f"{k} Damage"] for k in DAMAGE], "aux": [a["Poison Inflict"], a["Bleed Inflict"]]}

    def stats(self, iid):
        p = self.item.get(iid)
        out = {}
        if self.detail.get(iid):
            out["desc"] = self.detail[iid].strip()
        if self.simple.get(iid):
            out["effect"] = self.simple[iid].strip()
        if p is None:
            return out
        if p["Weapon ID"] > 0:
            out["weapon"] = self.weapon_stats(p["Weapon ID"])
        if p["Armor ID"] > 0:
            out["armor"] = self.armor_stats(p["Armor ID"])
        if p["Ring ID"] > 0 and (r := self.ring.get(p["Ring ID"])):
            out["ring"] = {"wt": num(r["Weight"]), "dur": num(r["Durability"])}
        if p["Spell ID"] > 0:
            out["spell"] = self.spell_stats(p["Spell ID"])
        if p["Ammunition ID"] > 0:
            out["ammo"] = self.arrow_stats(p["Ammunition ID"])
        return {k: v for k, v in out.items() if v is not None}


class _Empty:
    """A definition with no fields, for params read by hand."""

    fields, size = [], 0

    def read(self, data, off):
        return {}


def main(game, defs):
    items = json.load(open("src-tauri/data/items.json", encoding="utf-8"))
    g = Game(game, defs)
    out = {str(i["id"]): s for i in items if (s := g.stats(i["id"]))}
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    kinds = {k: sum(k in s for s in out.values()) for k in ("weapon", "armor", "ring", "spell", "ammo", "desc")}
    print(f"{OUT}: {len(out)} of {len(items)} items, {os.path.getsize(OUT) / 1e3:.0f} KB; {kinds}")
    renamed = [f"{i['id']} {i['name']} -> {g.names[i['id']]}" for i in items if g.names.get(i["id"], i["name"]) != i["name"]]
    if renamed:
        print("the game names these differently:", "; ".join(renamed))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
