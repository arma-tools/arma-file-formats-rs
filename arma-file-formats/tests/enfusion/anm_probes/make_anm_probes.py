# Writes the TXA sources of the anm probes, see README.md.
# usage: python make_anm_probes.py <output dir>
import math
import os
import sys

out = sys.argv[1]


def fmt(values):
    return " ".join("0" if v == 0 else repr(float(v)).rstrip("0").rstrip(".") for v in values)


def write(name, num_frames, bones, extra="", keys="t q s"):
    """bones: (name, depth, frames, kind) with frames {frame: {"t"|"q"|"s": values}}."""
    lines = ['$animation "" {', " #version 2", " #fps 30", " #numFrames %d" % num_frames]
    depth_open = 0
    for bone, depth, frames, kind in bones:
        while depth_open > depth:
            lines.append(" " * depth_open + "}")
            depth_open -= 1
        ind = " " * (depth + 1)
        lines.append(ind + '$%s "%s" {' % (kind, bone))
        lines.append(ind + " $keys %s {" % keys)
        for frame in sorted(frames):
            lines.append(ind + "  $frame %d {" % frame)
            for key in "tqs":
                if key in frames[frame]:
                    lines.append(ind + "   #%s %s" % (key, fmt(frames[frame][key])))
            lines.append(ind + "  }")
        lines.append(ind + " }")
        depth_open = depth + 1
    while depth_open > 0:
        lines.append(" " * depth_open + "}")
        depth_open -= 1
    lines.append(extra + "}")
    with open(os.path.join(out, name + ".txa"), "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def node(name, frames, depth=0, kind="node"):
    return (name, depth, frames, kind)


r = 0.7071068
t123 = {0: {"t": (0, 0, 0)}, 1: {"t": (1, 2, 3)}}

write("p_t", 1, [node("b", t123)])
write("p_tneg", 2, [node("b", {0: {"t": (-1, 0.5, 0)}, 1: {"t": (0, 0, 0)}, 2: {"t": (2, -0.5, 1)}})])
write("p_full5", 4, [node("b", {f: {"t": (f * 0.25, 1 - f * 0.25, (f % 2) * 0.5)} for f in range(5)})])
write("p_q1", 1, [node("b", {0: {"q": (0, r, 0, r)}, 1: {"q": (0, r, 0, r)}})])
write("p_tqs", 2, [node("b", {
    0: {"t": (0, 0, 0), "q": (0, 0, 0, 1), "s": (1, 1, 1)},
    1: {"t": (1, 2, 3), "q": (0, r, 0, r), "s": (2, 2, 2)},
    2: {"t": (3, 2, 1), "q": (r, 0, 0, r), "s": (1, 2, 4)},
})])
write("p_two", 1, [
    node("a", {0: {"t": (0, 0, 0)}, 1: {"t": (0, 1, 0)}}),
    node("c", {0: {"t": (0, 0, 0)}, 1: {"t": (0, 0, 2)}}, depth=1),
])
q_turn = {0: {"q": (0, 0, 0, 1)}, 1: {"q": (0, r, 0, r)}}
write("p_qs", 1, [node("b", q_turn)], keys="q s")
write("p_diff", 1, [node("b", q_turn, kind="nodeDiff")], keys="q s")

event = (
    '"AnimSrcEventGeneric \\"{64D50818623F4D11}\\" {\n Name \\"Ev\\"\n Comment \\"cmt\\"\n'
    ' MainPathOnly 1\n UserString \\"us\\"\n UserInt 7\n}"'
)
write("p_ev1", 1, [node("b", t123)],
      extra=' $events {\n  #event 0 "EvA" ' + event + ' 0 0 "" 0\n  #event 1 "EvB" "" 3 4 "str" 5\n }\n')
write("p_cp", 1, [node("b", t123)], extra=' $custProps {\n  #custProp "alpha" "one"\n  #custProp "b" ""\n }\n')

# Workbench drops keys that interpolation reproduces.
write("p_linear", 8, [node("b", {f: {"t": (f / 8, 0, 0)} for f in range(9)})])
write("p_hold", 8, [node("b", {f: {"t": (0 if f < 4 else 1, 0, 0)} for f in range(9)})])
write("p_slerp", 8, [node("b", {f: {"q": (0, math.sin(f / 16 * math.pi / 2), 0, math.cos(f / 16 * math.pi / 2))} for f in range(9)})])


def turn_y(degrees, f, frames=8):
    """Rotation about y, `degrees * f / frames`, as written by slerp sampling."""
    half = math.radians(degrees) * f / frames / 2
    return (0, math.sin(half), 0, math.cos(half))


# A one frame animation: all tracks have a key on every frame.
write("p_one", 0, [node("b", {0: {"t": (1, 2, 3), "q": (0, r, 0, r), "s": (2, 2, 2)}})])
# -160 degrees about y, the last key written as the equal quaternion of +200 degrees, whose dot
# product with the first key is negative. Reduced to two keys: interpolation takes the shorter arc.
short_arc = {f: {"q": turn_y(-160, f)} for f in range(8)}
short_arc[8] = {"q": turn_y(200, 8)}
write("p_shortarc", 8, [node("b", short_arc)])
