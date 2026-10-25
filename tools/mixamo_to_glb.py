"""Kev's Mixamo download -> game/assets/characters/shannon.glb (+ shannon.json).

Run headless with Blender 5.2:

    blender -b --factory-startup --python tools/mixamo_to_glb.py -- [--raw DIR] [--out FILE.glb] [--style toon|real]

--raw defaults to art/mixamo/raw next to this repo (git-ignored: shannon.fbx, anims/*.fbx and
"Soccer Game Pack.zip"); --out to game/assets/characters/shannon.glb. Both outputs are git-ignored:
Mixamo's licence lets us use the assets inside the game but not hand them out on their own, so only
this script is committed.

--style toon (the default, Kev's pick: Rematch is clean flat colours, not a photo) keeps the
proportions but not the detail: the hair cards and eyelashes go (the head's own painted hair takes
their place), the body under the clothes goes, every mesh is decimated to about 11k triangles in
all, and the photo textures become one flat colour per region, as materials the game recolours:
skin, hair (also the brows and eyes), shirt, shorts, socks, boots. --style real keeps Mixamo's
meshes and textures (shrunk to 1K).

What it does:
  1. imports the character, turns it to face the game's forward (glTF -Z) and applies the FBX's
     centimetre scale, so the armature and meshes are plain metres with no object transforms;
  2. imports each animation and retargets it onto the character's armature as an action named
     cleanly (run, sprint, pass...): the animation skeletons share the character's bone names and
     lengths but not its rest pose, so each bone's pose is matched in armature space;
  3. makes every clip in place: the hips' horizontal travel (the straight line of a cycle, the
     smoothed path of a one-shot) is taken out, the height kept; spins and turns also lose the
     hips' heading, which the game turns itself. What was taken out goes in the JSON sidecar, with
     each cycle's speed and foot phase and each kick's contact;
  4. styles the meshes (above), or for the realistic look shrinks the 4K textures and drops the
     specular and glossiness maps (Blender links the glossiness map as roughness, inverted), and
     exports one .glb with the meshes, materials and all the actions.

Prints the clips with their durations and frame counts.
"""

import json
import math
import os
import sys
import tempfile
import zipfile
from dataclasses import dataclass, field

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FPS = 30

# Textures: the camera stays a few metres away, so 1K is plenty for the 4K body maps.
TEXTURE_SIZE = 1024
HAIR_TEXTURE_SIZE = 512
ROUGHNESS = 0.65

# One-shots: the travel taken out is the hips' path smoothed over this long; what's left is the
# body's own sway over the feet.
SMOOTH_WINDOW = 0.5

# The toon style's regions: each mesh's share of triangles kept, and the flat colours (sRGB; the
# kit's are the prototype's blue kit, which the game recolours per team).
DECIMATE = {"Ch38_Body": 0.4, "Ch38_Shirt": 0.32, "Ch38_Shorts": 0.24, "Ch38_Shoes": 0.3, "Ch38_Socks": 0.45}
REGION_OF_MESH = {"Ch38_Shirt": "shirt", "Ch38_Shorts": "shorts", "Ch38_Socks": "socks", "Ch38_Shoes": "boots"}
COLOURS = {
    "skin": (0.80, 0.60, 0.47),
    "hair": (0.10, 0.07, 0.05),
    "shirt": (0.10, 0.30, 0.85),
    "shorts": (0.05, 0.10, 0.32),
    "socks": (0.92, 0.92, 0.90),
    "boots": (0.06, 0.06, 0.07),
}
# Above this height the body is the head; where its painted texture is darker than this, it's hair,
# brows or eyes.
HEAD_FROM = 1.5
DARK = 0.12


@dataclass
class Clip:
    name: str
    source: str  # "anims/<file>" or "pack/<file>" (inside the zip)
    loop: bool = False
    # The kicking foot ("R"/"L"): the contact is where its toes move fastest.
    kick: str = ""
    # Take the hips' heading out too (the game turns the body itself).
    strip_yaw: bool = False
    # Mirrored left for right: a kick with the other foot, a turn the other way.
    mirror: bool = False
    # Keep only these source frames (1-based, inclusive).
    frames: tuple = ()
    marks: dict = field(default_factory=dict)


CLIPS = [
    # Locomotion cycles.
    Clip("idle", "pack/offensive idle.fbx", loop=True),
    Clip("idle_ball", "anims/Soccer Idle.fbx", loop=True),
    Clip("walk", "anims/Standing Walk Forward.fbx", loop=True),
    Clip("walk_back", "anims/Walking Backwards.fbx", loop=True),
    Clip("run", "anims/Unarmed Run Forward.fbx", loop=True),
    Clip("run_back", "anims/Unarmed Run Back.fbx", loop=True),
    Clip("sprint", "anims/Fast Run.fbx", loop=True),
    # Lock move: the jogs in eight directions.
    Clip("jog", "pack/jog forward.fbx", loop=True),
    Clip("jog_back", "pack/jog backward.fbx", loop=True),
    Clip("jog_strafe_left", "pack/jog strafe left.fbx", loop=True),
    Clip("jog_strafe_right", "pack/jog strafe right.fbx", loop=True),
    Clip("jog_diag_left", "pack/jog forward diagonal (2).fbx", loop=True),
    Clip("jog_diag_right", "pack/jog forward diagonal.fbx", loop=True),
    Clip("jog_back_diag_left", "pack/jog backward diagonal (2).fbx", loop=True),
    Clip("jog_back_diag_right", "pack/jog backward diagonal.fbx", loop=True),
    # Kicks: the contact is where the kicking toes pass the standing foot.
    Clip("pass", "anims/Soccer Pass.fbx", kick="R"),
    Clip("chip", "anims/Chip.fbx", kick="R"),
    Clip("strike_jog", "pack/strike foward jog.fbx", kick="R"),
    Clip("strike_jog_left", "pack/strike foward jog.fbx", kick="L", mirror=True),
    Clip("penalty_kick", "pack/soccer penalty kick.fbx", kick="R"),
    # A run with the ball: its first touch (the right foot's sweep through under the hips) is the
    # push ball's and the dribble's.
    Clip("dribble", "anims/Dribble.fbx", kick="R", marks={"start": 0.2, "windup": 0.3, "contact": 0.45}),
    # Controls, moves and transitions.
    Clip("receive_chest", "anims/Receive.fbx"),
    Clip("receive", "pack/receive soccerball.fbx"),
    Clip("header", "anims/Header.fbx"),
    Clip("spin", "anims/Soccer Spin.fbx", strip_yaw=True),
    Clip("turn_180", "anims/Running Turn 180.fbx", strip_yaw=True),
    Clip("turn_180_left", "anims/Running Turn 180.fbx", strip_yaw=True, mirror=True),
    Clip("run_stop", "anims/Run To Stop.fbx", strip_yaw=True),
]


def mirrored(name):
    return name.replace("Left", "\0").replace("Right", "Left").replace("\0", "Right")


def args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opts = {"raw": os.path.join(REPO, "art", "mixamo", "raw"), "out": os.path.join(REPO, "game", "assets", "characters", "shannon.glb"), "style": "toon"}
    for i in range(0, len(argv) - 1, 2):
        opts[argv[i].lstrip("-")] = argv[i + 1]
    return opts


def short(name):
    """A bone's name without the rig's "mixamorigN:" prefix."""
    return name.split(":", 1)[-1]


def select_only(objects):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]


def import_character(path):
    bpy.ops.import_scene.fbx(filepath=path)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    # The FBX stands up (90 deg about X) in centimetres (0.01) facing Blender's -Y. Turned half
    # round it faces +Y, which the glTF exporter writes as -Z: the game's forward.
    arm.rotation_mode = "XYZ"
    arm.rotation_euler = (math.radians(90.0), 0.0, math.radians(180.0))
    select_only([arm] + meshes)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    # Triangles, so the exporter can write tangents for the normal maps.
    for m in meshes:
        bm = bmesh.new()
        bm.from_mesh(m.data)
        bmesh.ops.triangulate(bm, faces=bm.faces[:])
        bm.to_mesh(m.data)
        bm.free()
    # The T-pose action that comes with the character isn't a clip.
    if arm.animation_data:
        arm.animation_data.action = None
    for a in list(bpy.data.actions):
        bpy.data.actions.remove(a)
    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
    return arm, meshes


def deforming_bones(arm, meshes):
    """Bones that move a vertex, and their parents: the only ones worth animating."""
    names = {g.name for m in meshes for g in m.vertex_groups}
    keep = set()
    for b in arm.data.bones:
        if b.name in names:
            while b:
                keep.add(b.name)
                b = b.parent
    return [b for b in arm.data.bones if b.name in keep]


def unwrap(angles):
    out = [angles[0]]
    for a in angles[1:]:
        out.append(out[-1] + (a - out[-1] + math.pi) % math.tau - math.pi)
    return out


def smooth(values, window):
    """Centred moving average, the ends held."""
    n = len(values)
    half = max(1, int(round(window * FPS / 2)))
    padded = np.concatenate([np.repeat(values[:1], half, axis=0), values, np.repeat(values[-1:], half, axis=0)])
    kernel = np.ones(2 * half + 1) / (2 * half + 1)
    return np.stack([np.convolve(padded[:, i], kernel, mode="valid") for i in range(values.shape[1])], axis=1)[:n]


def sample_source(src_arm, frames, to_char):
    """Each frame's armature-space pose of every source bone, in the character's armature space
    (metres): {short name: [(location, rotation)]}."""
    out = {short(pb.name): [] for pb in src_arm.pose.bones}
    for f in frames:
        bpy.context.scene.frame_set(f)
        for pb in src_arm.pose.bones:
            loc, rot, _ = (to_char @ pb.matrix).decompose()
            out[short(pb.name)].append((loc.copy(), rot.copy()))
    return out


def retarget(clip, src_arm, char, bones):
    act = src_arm.animation_data.action
    f0, f1 = int(round(act.frame_range[0])), int(round(act.frame_range[1]))
    if clip.frames:
        f0, f1 = clip.frames
    frames = list(range(f0, f1 + 1))
    n = len(frames)
    # The source armature as the character's: the same half turn, its own object transform.
    to_char = Matrix.Rotation(math.pi, 4, "Z") @ src_arm.matrix_world
    pose = sample_source(src_arm, frames, to_char)
    if clip.mirror:
        # Each bone takes its counterpart's turn from the rest pose, reflected across the body's
        # middle (x = 0); the hips' path and heading reflect with it.
        rest_all = {short(b.name): b.matrix_local.to_quaternion() for b in char.data.bones}

        def flip(q):
            return Quaternion((q.w, q.x, -q.y, -q.z))

        pose = {
            name: [(Vector((-loc.x, loc.y, loc.z)), flip(rot @ rest_all[mirrored(name)].inverted()) @ rest_all[name]) for loc, rot in pose[mirrored(name)]]
            for name in pose
            if mirrored(name) in pose and name in rest_all and mirrored(name) in rest_all
        }
    hips = next(b for b in bones if b.parent is None)
    hips_name = short(hips.name)
    rest_rot = {b.name: b.matrix_local.to_quaternion() for b in bones}
    # Facing +Y, right +X, up +Z.
    hips_loc = np.array([[p[0].x, p[0].y, p[0].z] for p in pose[hips_name]])
    hips_rest = rest_rot[hips.name]
    heading = unwrap([math.atan2(-(r @ hips_rest.inverted() @ Vector((0, 1, 0))).x, (r @ hips_rest.inverted() @ Vector((0, 1, 0))).y) for _, r in pose[hips_name]])
    heading = np.array(heading) - heading[0]

    # What a foot does, from the source (travel included), for the marks.
    def toes(side):
        return np.array([[p[0].x, p[0].y, p[0].z] for p in pose[side + "ToeBase"]])

    t = np.arange(n) / FPS
    if clip.loop:
        removed = (hips_loc[-1, :2] - hips_loc[0, :2])[None, :] * (t / t[-1])[:, None]
    else:
        path = smooth(hips_loc[:, :2], SMOOTH_WINDOW)
        removed = path - path[0]
    yaw_removed = heading if clip.strip_yaw else np.zeros(n)

    marks = dict(clip.marks)
    if clip.kick and "contact" not in marks:
        side, other = ("Right", "Left") if clip.kick == "R" else ("Left", "Right")
        toe, stand = toes(side), toes(other)
        speed = np.linalg.norm(np.diff(toe, axis=0), axis=1) * FPS
        peak = int(np.argmax(speed))
        # The top of the backswing: the toes furthest behind the hips in the half second before
        # the foot is fastest; the contact, where they then pass the standing foot.
        lo = max(0, peak - FPS // 2)
        ahead = toe[:, 1] - hips_loc[:, 1]
        windup = lo + int(np.argmin(ahead[lo:peak + 1]))
        gap = toe[:, 1] - stand[:, 1]
        contact = (peak + 0.5) / FPS
        for k in range(windup + 1, n):
            if gap[k] >= 0 > gap[k - 1]:
                contact = (k - 1 - gap[k - 1] / (gap[k] - gap[k - 1])) / FPS
                break
        marks["windup"] = round(float(windup / FPS), 4)
        marks["contact"] = round(float(contact), 4)
        marks["foot_speed"] = round(float(speed[peak]), 2)
    elif clip.kick:
        speed = np.linalg.norm(np.diff(toes("Right" if clip.kick == "R" else "Left"), axis=0), axis=1) * FPS
        marks["foot_speed"] = round(float(speed.max()), 2)
    sync = None
    if clip.loop:
        # The cycle's phase where the right foot is down and the left highest: cycles blend in step.
        lift = toes("Left")[:, 2] - toes("Right")[:, 2]
        sync = round(float(int(np.argmax(lift[:-1])) / (n - 1)), 4)

    # The new action, keyed on the character's bones: each bone turned to the source's
    # armature-space rotation; the hips also placed (less the travel taken out).
    action = bpy.data.actions.new(clip.name)
    slot = action.slots.new(id_type="OBJECT", name=char.name)
    bag = action.layers.new("Layer").strips.new(type="KEYFRAME").channelbags.new(slot)
    for b in bones:
        src = pose.get(short(b.name))
        if src is None:
            continue
        quats = []
        locs = []
        for k in range(n):
            loc, rot = src[k]
            if b.parent is None:
                yaw = Quaternion((0, 0, 1), -yaw_removed[k])
                rot = yaw @ rot
                loc = Vector((loc.x - removed[k, 0], loc.y - removed[k, 1], loc.z))
                basis = b.matrix_local.inverted() @ Matrix.Translation(loc) @ rot.to_matrix().to_4x4()
                bl, q, _ = basis.decompose()
                locs.append(bl)
            else:
                # Relative to the parent, so a heading taken off the hips turns the whole body.
                parent_rot = pose[short(b.parent.name)][k][1]
                rel = rest_rot[b.parent.name].inverted() @ rest_rot[b.name]
                q = rel.inverted() @ parent_rot.inverted() @ rot
            if quats and quats[-1].dot(q) < 0:
                q = -q
            quats.append(q)
        path = f'pose.bones["{b.name}"]'
        for i in range(4):
            key(bag, path + ".rotation_quaternion", i, [q[i] for q in quats])
        if locs:
            for i in range(3):
                key(bag, path + ".location", i, [v[i] for v in locs])

    def r(v):
        return round(float(v), 4)

    travel = hips_loc[-1, :2] - hips_loc[0, :2]
    meta = {
        "duration": r((n - 1) / FPS),
        "frames": n,
        "loop": clip.loop,
        "speed": r(np.linalg.norm(travel) / ((n - 1) / FPS)) if clip.loop else 0.0,
        # Degrees from ahead, + right.
        "dir": r(math.degrees(math.atan2(travel[0], travel[1]))) if clip.loop and np.linalg.norm(travel) > 0.05 else 0.0,
        "travel": [r(travel[0]), r(travel[1])],
        "turn": r(math.degrees(heading[-1])),
    }
    if sync is not None:
        meta["sync"] = sync
    meta.update(marks)
    if not clip.loop:
        # The travel and turn taken out, per frame: [right, ahead] metres and degrees (left +).
        meta["root"] = [[r(removed[k, 0]), r(removed[k, 1]), r(math.degrees(yaw_removed[k]))] for k in range(n)]
    return action, meta


def key(bag, path, index, values):
    fc = bag.fcurves.new(path, index=index)
    fc.keyframe_points.add(len(values))
    co = np.empty(2 * len(values), dtype=np.float32)
    co[0::2] = np.arange(len(values))
    co[1::2] = values
    fc.keyframe_points.foreach_set("co", co)
    fc.update()


def shrink_textures(tmp):
    """Each material's base colour and normal map at a game size, as plain PNG files; the
    specular and glossiness maps go (a flat roughness instead)."""
    for mat in bpy.data.materials:
        if not mat.node_tree:
            continue
        nodes, links = mat.node_tree.nodes, mat.node_tree.links
        bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
        for socket in ("Specular IOR Level", "Roughness"):
            for link in list(bsdf.inputs[socket].links):
                nodes.remove(link.from_node)
        bsdf.inputs["Roughness"].default_value = ROUGHNESS
        bsdf.inputs["Specular IOR Level"].default_value = 0.5
    # The FBX loads the hair's map once per use: one image per file, so its colour and alpha
    # export as one RGBA texture.
    by_file = {}
    for im in list(bpy.data.images):
        first = by_file.setdefault(im.filepath, im)
        if first is not im:
            im.user_remap(first)
    size_of = {}
    for mat in bpy.data.materials:
        if mat.node_tree:
            for n in mat.node_tree.nodes:
                if n.type == "TEX_IMAGE" and n.image:
                    hair = "hair" in mat.name.lower()
                    size_of[n.image.name] = (HAIR_TEXTURE_SIZE if hair else TEXTURE_SIZE, hair)
    for name, (size, alpha) in size_of.items():
        old = bpy.data.images[name]
        w, h = old.size
        if max(w, h) > size:
            old.scale(size, size * h // w)
        w, h = old.size
        px = np.empty(w * h * 4, dtype=np.float32)
        old.pixels.foreach_get(px)
        new = bpy.data.images.new(name + "_game", w, h, alpha=alpha)
        new.colorspace_settings.name = old.colorspace_settings.name
        new.pixels.foreach_set(px)
        path = os.path.join(tmp, f"{len(os.listdir(tmp))}.png")
        new.filepath_raw = path
        new.file_format = "PNG"
        new.save()
        new.source = "FILE"
        new.reload()
        old.user_remap(new)
    for im in list(bpy.data.images):
        if im.users == 0:
            bpy.data.images.remove(im)


def srgb_to_linear(c):
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


def flat_material(name):
    mat = bpy.data.materials.new(name)
    bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Base Color"].default_value = (*srgb_to_linear(COLOURS[name]), 1.0)
    bsdf.inputs["Roughness"].default_value = 0.8
    bsdf.inputs["Specular IOR Level"].default_value = 0.3
    return mat


def base_colour_pixels(mat):
    """A material's base colour texture as an (h, w, 4) array."""
    bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    image = bsdf.inputs["Base Color"].links[0].from_node.image
    w, h = image.size
    px = np.empty(w * h * 4, dtype=np.float32)
    image.pixels.foreach_get(px)
    return px.reshape(h, w, 4)


def apply_modifier(obj, mod):
    with bpy.context.temp_override(object=obj, active_object=obj):
        bpy.ops.object.modifier_move_to_index(modifier=mod.name, index=0)
        bpy.ops.object.modifier_apply(modifier=mod.name)


def stylise(meshes):
    """The toon style (see the notes at the top). Returns the meshes kept."""
    from mathutils.bvhtree import BVHTree

    for m in [m for m in meshes if m.name not in DECIMATE]:
        bpy.data.objects.remove(m, do_unlink=True)
    meshes = [m for m in bpy.data.objects if m.type == "MESH"]
    body = next(m for m in meshes if m.name == "Ch38_Body")
    cloth = [m for m in meshes if m.name in REGION_OF_MESH]
    mats = {name: flat_material(name) for name in COLOURS}

    # The body: skin, and hair where the head's painted texture is dark (the scalp under the hair
    # cards, the brows, the eyes); the faces under the clothes go (a ray out from them meets cloth
    # within 5 cm), so nothing pokes through and no triangles are spent unseen.
    px = base_colour_pixels(body.data.materials[0])
    h, w = px.shape[:2]
    # (Material slots first: a face can't keep an index past the slots there are.)
    body.data.materials.clear()
    body.data.materials.append(mats["skin"])
    body.data.materials.append(mats["hair"])
    trees = []
    for m in cloth:
        cbm = bmesh.new()
        cbm.from_mesh(m.data)
        trees.append(BVHTree.FromBMesh(cbm))
        cbm.free()
    bm = bmesh.new()
    bm.from_mesh(body.data)
    uv = bm.loops.layers.uv.active
    hidden = []
    for f in bm.faces:
        c = f.calc_center_median()
        f.material_index = 0
        if c.z < HEAD_FROM - 0.05:
            if any(t.ray_cast(c + f.normal * 1e-3, f.normal, 0.05)[0] is not None for t in trees):
                hidden.append(f)
        elif c.z >= HEAD_FROM:
            st = sum((loop[uv].uv for loop in f.loops), Vector((0.0, 0.0))) / len(f.loops)
            r, g, b, _ = px[int((st.y % 1.0) * (h - 1)), int((st.x % 1.0) * (w - 1))]
            if 0.2126 * r + 0.7152 * g + 0.0722 * b < DARK:
                f.material_index = 1
    bmesh.ops.delete(bm, geom=hidden, context="FACES")
    bm.to_mesh(body.data)
    bm.free()
    print(f"body: {len(hidden)} faces under the clothes gone, {sum(p.material_index == 1 for p in body.data.polygons)} hair faces")
    for m in cloth:
        m.data.materials.clear()
        m.data.materials.append(mats[REGION_OF_MESH[m.name]])
        for p in m.data.polygons:
            p.material_index = 0

    for m in meshes:
        mod = m.modifiers.new("decimate", "DECIMATE")
        mod.ratio = DECIMATE[m.name]
        mod.use_collapse_triangulate = True
        apply_modifier(m, mod)
        # Flat colours: no UVs needed.
        while m.data.uv_layers:
            m.data.uv_layers.remove(m.data.uv_layers[0])

    for mat in [mat for mat in bpy.data.materials if mat.name not in COLOURS]:
        bpy.data.materials.remove(mat)
    for im in list(bpy.data.images):
        bpy.data.images.remove(im)
    return meshes


def main():
    opts = args()
    raw, out = opts["raw"], opts["out"]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps = FPS
    char, meshes = import_character(os.path.join(raw, "shannon.fbx"))
    bones = deforming_bones(char, meshes)
    tmp = tempfile.mkdtemp(prefix="mixamo_")
    pack = zipfile.ZipFile(os.path.join(raw, "Soccer Game Pack.zip"))
    actions, meta = [], {}
    for clip in CLIPS:
        kind, file = clip.source.split("/", 1)
        if kind == "pack":
            path = pack.extract(file, tmp)
        else:
            path = os.path.join(raw, "anims", file)
        before = set(bpy.data.objects)
        before_actions = set(bpy.data.actions)
        bpy.ops.import_scene.fbx(filepath=path)
        new = [o for o in bpy.data.objects if o not in before]
        src = next(o for o in new if o.type == "ARMATURE")
        action, info = retarget(clip, src, char, bones)
        actions.append(action)
        meta[clip.name] = info
        for o in new:
            bpy.data.objects.remove(o, do_unlink=True)
        for a in set(bpy.data.actions) - before_actions - {action}:
            bpy.data.actions.remove(a)
    for a in list(bpy.data.armatures):
        if a.users == 0:
            bpy.data.armatures.remove(a)

    # Every action on its own NLA track: the exporter writes each as a glTF animation.
    ad = char.animation_data_create()
    for action in actions:
        track = ad.nla_tracks.new()
        track.name = action.name
        strip = track.strips.new(action.name, 0, action)
        strip.action_slot = action.slots[0]

    if opts["style"] == "toon":
        meshes = stylise(meshes)
    else:
        tex = os.path.join(tmp, "tex")
        os.makedirs(tex)
        shrink_textures(tex)

    os.makedirs(os.path.dirname(out), exist_ok=True)
    select_only([char] + meshes)
    bpy.ops.export_scene.gltf(
        filepath=out,
        export_format="GLB",
        use_selection=True,
        export_animations=True,
        export_animation_mode="ACTIONS",
        export_anim_slide_to_zero=True,
        export_force_sampling=True,
        export_optimize_animation_size=True,
        export_skins=True,
        export_morph=False,
        # Only the realistic look has normal maps.
        export_tangents=opts["style"] != "toon",
        export_image_format="AUTO",
        export_yup=True,
        export_apply=False,
    )
    side = os.path.splitext(out)[0] + ".json"
    with open(side, "w") as f:
        json.dump({"fps": FPS, "clips": meta}, f, indent=1)

    triangles = sum(len(p.vertices) - 2 for m in meshes for p in m.data.polygons)
    print(f"\n{out}: {opts['style']}, {os.path.getsize(out) / 1e6:.1f} MB, {triangles} triangles, {len(bones)} animated bones")
    print(f"{'clip':22} {'secs':>5} {'frames':>6}  notes")
    for name, m in meta.items():
        notes = []
        if m["loop"]:
            notes.append(f"loop {m['speed']:.2f} m/s at {m['dir']:+.0f} deg, sync {m['sync']:.2f}")
        else:
            notes.append(f"travel ({m['travel'][0]:+.2f}, {m['travel'][1]:+.2f}) m, turn {m['turn']:+.0f} deg")
        if "contact" in m:
            notes.append(f"contact {m['contact']:.2f}s windup {m['windup']:.2f}s ({m['foot_speed']} m/s)")
        print(f"{name:22} {m['duration']:5.2f} {m['frames']:6d}  {'; '.join(notes)}")


main()
