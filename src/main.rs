use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
};

use rbx_binary;
use rbx_dom_weak::{Ustr, WeakDom};
use rbx_types::{CFrame, Matrix3, Ref, Variant, Vector3};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <place.rbxl>", args[0]);
        return Ok(());
    }

    let path = PathBuf::from(&args[1]);
    let file = BufReader::new(File::open(&path)?);
    let dom: WeakDom = rbx_binary::from_reader(file)?;

    let mut obj_path = path.clone();
    obj_path.set_extension("obj");
    let mut mtl_path = path.clone();
    mtl_path.set_extension("mtl");

    let mut obj = BufWriter::new(File::create(&obj_path)?);
    let mut mtl = BufWriter::new(File::create(&mtl_path)?);

    writeln!(obj, "# Exported from Roblox place")?;
    writeln!(
        obj,
        "mtllib {}",
        mtl_path.file_name().unwrap().to_string_lossy()
    )?;

    let mut vertex_offset = 0;
    let mut material_map: HashMap<(u8, u8, u8, u8), String> = HashMap::new();
    let mut next_mat_id = 0;

    for &child_ref in dom.root().children() {
        export_instance(
            &dom,
            child_ref,
            &mut obj,
            &mut mtl,
            &mut vertex_offset,
            &mut material_map,
            &mut next_mat_id,
        )?;
    }

    Ok(())
}

fn export_instance(
    dom: &WeakDom,
    inst_ref: Ref,
    obj: &mut dyn Write,
    mtl: &mut dyn Write,
    vertex_offset: &mut usize,
    material_map: &mut HashMap<(u8, u8, u8, u8), String>,
    next_mat_id: &mut usize,
) -> Result<(), Box<dyn Error>> {
    let inst = dom.get_by_ref(inst_ref).unwrap();

    match inst.class.as_str() {
        "Part" | "WedgePart" | "CornerWedgePart" => {
            let size = match inst.properties.get(&Ustr::from("Size")) {
                Some(Variant::Vector3(v)) => *v,
                _ => Vector3::new(1.0, 1.0, 1.0),
            };

            let cframe = match inst.properties.get(&Ustr::from("CFrame")) {
                Some(Variant::CFrame(cf)) => *cf,
                _ => CFrame {
                    position: Vector3::new(0.0, 0.0, 0.0),
                    orientation: Matrix3::identity(),
                },
            };

            let (r, g, b) = match inst.properties.get(&Ustr::from("Color")) {
                Some(Variant::Color3uint8(c)) => (c.r, c.g, c.b),
                _ => (255, 255, 255),
            };

            let transparency = match inst.properties.get(&Ustr::from("Transparency")) {
                Some(Variant::Float32(t)) => *t,
                _ => 0.0,
            };
            let a = ((1.0 - transparency) * 255.0) as u8;

            let mat_key = (r, g, b, a);
            let mat_name = material_map.entry(mat_key).or_insert_with(|| {
                let name = format!("mat_{}", *next_mat_id);
                *next_mat_id += 1;
                let (rf, gf, bf, af) = (
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    a as f32 / 255.0,
                );
                writeln!(mtl, "newmtl {}", name).unwrap();
                writeln!(mtl, "Kd {} {} {}", rf, gf, bf).unwrap();
                writeln!(mtl, "d {}", af).unwrap();
                writeln!(mtl).unwrap();
                name
            });

            writeln!(obj, "usemtl {}", mat_name)?;

            let (local_vertices, local_faces) = match inst.class.as_str() {
                "Part" => {
                    let shape = match inst.properties.get(&Ustr::from("Shape")) {
                        Some(Variant::Enum(e)) => e.to_u32(),
                        _ => 1,
                    };
                    match shape {
                        0 => sphere_mesh(size, 2, 0),
                        1 => cube_mesh(size),
                        2 => cylinder_mesh(size, 24),
                        _ => cube_mesh(size),
                    }
                }
                "WedgePart" => wedge_mesh(size),
                "CornerWedgePart" => corner_wedge_mesh(size),
                _ => cube_mesh(size),
            };

            for v in local_vertices.iter() {
                let pos = apply_cframe(*v, &cframe);
                writeln!(obj, "v {} {} {}", pos.x, pos.y, pos.z)?;
            }

            for f in local_faces.iter() {
                writeln!(
                    obj,
                    "f {} {} {}",
                    f.0 + *vertex_offset + 1,
                    f.1 + *vertex_offset + 1,
                    f.2 + *vertex_offset + 1
                )?;
            }

            *vertex_offset += local_vertices.len();
        }
        _ => {}
    }

    for &child_ref in inst.children() {
        export_instance(
            dom,
            child_ref,
            obj,
            mtl,
            vertex_offset,
            material_map,
            next_mat_id,
        )?;
    }

    Ok(())
}

fn apply_matrix3(m: &Matrix3, v: Vector3) -> Vector3 {
    Vector3::new(
        m.x.x * v.x + m.x.y * v.y + m.x.z * v.z,
        m.y.x * v.x + m.y.y * v.y + m.y.z * v.z,
        m.z.x * v.x + m.z.y * v.y + m.z.z * v.z,
    )
}

fn apply_cframe(v: Vector3, cf: &CFrame) -> Vector3 {
    let r = apply_matrix3(&cf.orientation, v);
    Vector3::new(
        r.x + cf.position.x,
        r.y + cf.position.y,
        r.z + cf.position.z,
    )
}

fn cube_mesh(size: Vector3) -> (Vec<Vector3>, Vec<(usize, usize, usize)>) {
    let sx = size.x / 2.0;
    let sy = size.y / 2.0;
    let sz = size.z / 2.0;

    let vertices = vec![
        Vector3::new(-sx, -sy, -sz),
        Vector3::new(sx, -sy, -sz),
        Vector3::new(sx, sy, -sz),
        Vector3::new(-sx, sy, -sz),
        Vector3::new(-sx, -sy, sz),
        Vector3::new(sx, -sy, sz),
        Vector3::new(sx, sy, sz),
        Vector3::new(-sx, sy, sz),
    ];

    let faces = vec![
        (0, 1, 2),
        (0, 2, 3),
        (4, 5, 6),
        (4, 6, 7),
        (0, 1, 5),
        (0, 5, 4),
        (1, 2, 6),
        (1, 6, 5),
        (2, 3, 7),
        (2, 7, 6),
        (3, 0, 4),
        (3, 4, 7),
    ];

    (vertices, faces)
}

fn sphere_mesh(
    size: Vector3,
    subdivisions: usize,
    _unused: usize,
) -> (Vec<Vector3>, Vec<(usize, usize, usize)>) {
    let radius_x = size.x / 2.0;
    let radius_y = size.y / 2.0;
    let radius_z = size.z / 2.0;

    let t = (1.0 + 5.0f32.sqrt()) / 2.0;

    let mut vertices = vec![
        Vector3::new(-1.0, t, 0.0),
        Vector3::new(1.0, t, 0.0),
        Vector3::new(-1.0, -t, 0.0),
        Vector3::new(1.0, -t, 0.0),
        Vector3::new(0.0, -1.0, t),
        Vector3::new(0.0, 1.0, t),
        Vector3::new(0.0, -1.0, -t),
        Vector3::new(0.0, 1.0, -t),
        Vector3::new(t, 0.0, -1.0),
        Vector3::new(t, 0.0, 1.0),
        Vector3::new(-t, 0.0, -1.0),
        Vector3::new(-t, 0.0, 1.0),
    ];

    let mut faces = vec![
        (0, 11, 5),
        (0, 5, 1),
        (0, 1, 7),
        (0, 7, 10),
        (0, 10, 11),
        (1, 5, 9),
        (5, 11, 4),
        (11, 10, 2),
        (10, 7, 6),
        (7, 1, 8),
        (3, 9, 4),
        (3, 4, 2),
        (3, 2, 6),
        (3, 6, 8),
        (3, 8, 9),
        (4, 9, 5),
        (2, 4, 11),
        (6, 2, 10),
        (8, 6, 7),
        (9, 8, 1),
    ];

    for v in vertices.iter_mut() {
        let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        v.x /= len;
        v.y /= len;
        v.z /= len;
    }

    for _ in 0..subdivisions {
        let mut new_faces = Vec::new();
        let mut mid_cache = HashMap::<(usize, usize), usize>::new();

        let get_midpoint = |a: usize,
                            b: usize,
                            vertices: &mut Vec<Vector3>,
                            cache: &mut HashMap<(usize, usize), usize>|
         -> usize {
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(&idx) = cache.get(&key) {
                return idx;
            }
            let va = vertices[a];
            let vb = vertices[b];
            let mut vm = Vector3::new(
                (va.x + vb.x) / 2.0,
                (va.y + vb.y) / 2.0,
                (va.z + vb.z) / 2.0,
            );
            let len = (vm.x * vm.x + vm.y * vm.y + vm.z * vm.z).sqrt();
            vm.x /= len;
            vm.y /= len;
            vm.z /= len;
            let idx = vertices.len();
            vertices.push(vm);
            cache.insert(key, idx);
            idx
        };

        for &(a, b, c) in faces.iter() {
            let ab = get_midpoint(a, b, &mut vertices, &mut mid_cache);
            let bc = get_midpoint(b, c, &mut vertices, &mut mid_cache);
            let ca = get_midpoint(c, a, &mut vertices, &mut mid_cache);
            new_faces.push((a, ab, ca));
            new_faces.push((b, bc, ab));
            new_faces.push((c, ca, bc));
            new_faces.push((ab, bc, ca));
        }

        faces = new_faces;
    }

    for v in vertices.iter_mut() {
        v.x *= radius_x;
        v.y *= radius_y;
        v.z *= radius_z;
    }

    (vertices, faces)
}

fn cylinder_mesh(size: Vector3, steps: usize) -> (Vec<Vector3>, Vec<(usize, usize, usize)>) {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();

    let x_half = size.x / 2.0;
    let y_half = size.y / 2.0;
    let z_half = size.z / 2.0;

    for i in 0..steps {
        let theta = 2.0 * std::f32::consts::PI * i as f32 / steps as f32;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        vertices.push(Vector3::new(
            -x_half,
            y_half * cos_theta,
            z_half * sin_theta,
        ));
        vertices.push(Vector3::new(x_half, y_half * cos_theta, z_half * sin_theta));
    }

    vertices.push(Vector3::new(-x_half, 0.0, 0.0));
    vertices.push(Vector3::new(x_half, 0.0, 0.0));

    for i in 0..steps {
        let next = (i + 1) % steps;
        faces.push((i * 2, next * 2, next * 2 + 1));
        faces.push((i * 2, next * 2 + 1, i * 2 + 1));
        faces.push((i * 2, next * 2, vertices.len() - 2));
        faces.push((i * 2 + 1, next * 2 + 1, vertices.len() - 1));
    }

    (vertices, faces)
}

fn wedge_mesh(size: Vector3) -> (Vec<Vector3>, Vec<(usize, usize, usize)>) {
    let sx = size.x / 2.0;
    let sy = size.y / 2.0;
    let sz = size.z / 2.0;

    let vertices = vec![
        Vector3::new(-sx, -sy, -sz),
        Vector3::new(sx, -sy, -sz),
        Vector3::new(sx, -sy, sz),
        Vector3::new(-sx, -sy, sz),
        Vector3::new(-sx, sy, sz),
        Vector3::new(sx, sy, sz),
    ];

    let faces = vec![
        (0, 1, 2),
        (0, 2, 3),
        (0, 1, 4),
        (1, 5, 4),
        (3, 2, 5),
        (3, 5, 4),
        (0, 3, 4),
        (1, 2, 5),
    ];

    (vertices, faces)
}

fn corner_wedge_mesh(size: Vector3) -> (Vec<Vector3>, Vec<(usize, usize, usize)>) {
    wedge_mesh(size)
}
