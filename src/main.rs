use std::{
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

    let mut out_path = path.clone();
    out_path.set_extension("obj");
    let out_file = BufWriter::new(File::create(&out_path)?);

    let mut vertices: Vec<(Vector3, Vector3)> = Vec::new();
    let mut faces: Vec<(usize, usize, usize)> = Vec::new();
    let mut vertex_offset = 0;

    for &child_ref in dom.root().children() {
        collect_mesh(
            &dom,
            child_ref,
            &mut vertices,
            &mut faces,
            &mut vertex_offset,
        )?;
    }

    let mut out_file = out_file;

    writeln!(out_file, "# Exported from Roblox place")?;

    for (pos, color) in vertices.iter() {
        writeln!(
            out_file,
            "v {} {} {} {} {} {}",
            pos.x, pos.y, pos.z, color.x, color.y, color.z
        )?;
    }

    for f in faces.iter() {
        writeln!(out_file, "f {} {} {}", f.0 + 1, f.1 + 1, f.2 + 1)?;
    }

    Ok(())
}

fn collect_mesh(
    dom: &WeakDom,
    inst_ref: Ref,
    vertices: &mut Vec<(Vector3, Vector3)>,
    faces: &mut Vec<(usize, usize, usize)>,
    vertex_offset: &mut usize,
) -> Result<(), Box<dyn Error>> {
    let inst = dom.get_by_ref(inst_ref).unwrap();
    let class = &inst.class;

    if class.as_str() == "Part"
        || class.as_str() == "WedgePart"
        || class.as_str() == "CornerWedgePart"
    {
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

        let color = match inst.properties.get(&Ustr::from("Color")) {
            Some(Variant::Color3uint8(c)) => {
                Vector3::new(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0)
            }
            _ => Vector3::new(1.0, 1.0, 1.0),
        };

        let (local_vertices, local_faces) = generate_mesh(class, size);

        for v in local_vertices.iter() {
            let pos = apply_cframe(*v, &cframe);
            vertices.push((pos, color));
        }

        for f in local_faces.iter() {
            faces.push((
                f.0 + *vertex_offset,
                f.1 + *vertex_offset,
                f.2 + *vertex_offset,
            ));
        }

        *vertex_offset += local_vertices.len();
    }

    for &child_ref in inst.children() {
        collect_mesh(dom, child_ref, vertices, faces, vertex_offset)?;
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

fn generate_mesh(class: &Ustr, size: Vector3) -> (Vec<Vector3>, Vec<(usize, usize, usize)>) {
    match class.as_str() {
        "Part" => cube_mesh(size),
        "WedgePart" => wedge_mesh(size),
        "CornerWedgePart" => wedge_mesh(size),
        _ => cube_mesh(size),
    }
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
