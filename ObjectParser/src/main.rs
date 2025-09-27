use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
struct Vec2 { x: f32, y: f32 }
#[derive(Debug, Clone, Copy)]
struct Vec3 { x: f32, y: f32, z: f32 }

#[derive(Debug)]

//Structura para almacenar datos del OBJ
struct ObjData {
    vertices: Vec<Vec3>,
    uvs: Vec<Vec2>,
    normals: Vec<Vec3>,
    faces: Vec<String>,       // "v/vt/vn v/vt/vn ..."
    mtllibs: Vec<String>,
    used_materials: Vec<String>,
}
// Función para parsear un archivo OBJ
fn parse_obj(path: &PathBuf) -> std::io::Result<ObjData> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut vertices = Vec::<Vec3>::new();
    let mut uvs = Vec::<Vec2>::new();
    let mut normals = Vec::<Vec3>::new();
    let mut faces = Vec::<String>::new();
    let mut mtllibs = Vec::<String>::new();
    let mut used_materials = Vec::<String>::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let mut it = line.split_whitespace();
        if let Some(tag) = it.next() {
            match tag {
                "v" => {
                    let vals: Vec<f32> = it.filter_map(|s| s.parse::<f32>().ok()).collect();
                    if vals.len() >= 3 { vertices.push(Vec3 { x: vals[0], y: vals[1], z: vals[2] }); }
                },
                "vt" => {
                    let vals: Vec<f32> = it.filter_map(|s| s.parse::<f32>().ok()).collect();
                    if vals.len() >= 2 { uvs.push(Vec2 { x: vals[0], y: vals[1] }); }
                },
                "vn" => {
                    let vals: Vec<f32> = it.filter_map(|s| s.parse::<f32>().ok()).collect();
                    if vals.len() >= 3 { normals.push(Vec3 { x: vals[0], y: vals[1], z: vals[2] }); }
                },
                "f" => faces.push(it.collect::<Vec<&str>>().join(" ")),
                "mtllib" => mtllibs.push(it.collect::<Vec<&str>>().join(" ")),
                "usemtl" => used_materials.push(it.collect::<Vec<&str>>().join(" ")),
                _ => {}
            }
        }
    }
    Ok(ObjData { vertices, uvs, normals, faces, mtllibs, used_materials })
}

/* ---------- BMP + raster 2D ---------- */
fn save_bmp(path: &str, width: usize, height: usize, pixels: &Vec<u8>) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    let row_stride_unpadded = width * 3;
    let padding = (4 - (row_stride_unpadded as u32 % 4)) % 4;
    let row_stride = row_stride_unpadded as u32 + padding;
    let pixel_array_size = row_stride * height as u32;
    let file_size = 14 + 40 + pixel_array_size;

    // BMP header
    f.write_all(&[0x42, 0x4D])?;
    f.write_all(&file_size.to_le_bytes())?;
    f.write_all(&[0u8; 4])?;
    f.write_all(&(14 + 40 as i32 as u32).to_le_bytes())?;
    // DIB
    f.write_all(&40u32.to_le_bytes())?;
    f.write_all(&(width as i32).to_le_bytes())?;
    f.write_all(&(-(height as i32)).to_le_bytes())?; // top-down
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&24u16.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?;
    f.write_all(&pixel_array_size.to_le_bytes())?;
    f.write_all(&[0u8; 16])?;

    // Pixel data (BGR + padding)
    let mut row = vec![0u8; row_stride as usize];
    for y in 0..height {
        let start = y * row_stride_unpadded;
        for x in 0..width {
            let i = start + x*3;
            row[x*3 + 0] = pixels[i+2];
            row[x*3 + 1] = pixels[i+1];
            row[x*3 + 2] = pixels[i+0];
        }
        for p in (row_stride_unpadded)..(row_stride as usize) { row[p] = 0; }
        f.write_all(&row)?;
    }
    Ok(())
}

struct Image { w: usize, h: usize, data: Vec<u8> }
impl Image {
    fn new(w: usize, h: usize) -> Self { Self { w, h, data: vec![255; w*h*3] } } // fondo blanco
    fn set(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 { return; }
        let idx = ((y as usize) * self.w + (x as usize)) * 3;
        self.data[idx] = r; self.data[idx+1] = g; self.data[idx+2] = b;
    }
}
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax)*(by - ay) - (py - ay)*(bx - ax)
}
fn fill_triangle(img: &mut Image, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, r: u8, g: u8, b: u8) {
    let min_x = x0.min(x1).min(x2).floor().max(0.0) as i32;
    let max_x = x0.max(x1).max(x2).ceil().min((img.w-1) as f32) as i32;
    let min_y = y0.min(y1).min(y2).floor().max(0.0) as i32;
    let max_y = y0.max(y1).max(y2).ceil().min((img.h-1) as f32) as i32;
    let area = edge(x0,y0,x1,y1,x2,y2);
    if area == 0.0 { return; }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(x1,y1,x2,y2,px,py);
            let w1 = edge(x2,y2,x0,y0,px,py);
            let w2 = edge(x0,y0,x1,y1,px,py);
            if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                img.set(x,y,r,g,b);
            }
        }
    }
}

/* ---------- Utilidades para caras y proyección ---------- */
// Devuelve índices v (1-based en OBJ) como 0-based; ignora vt/vn
fn parse_face_vertex_indices(face: &str) -> Vec<usize> {
    face.split_whitespace().filter_map(|tok| {
        let mut parts = tok.split('/');
        if let Some(v) = parts.next() {
            if let Ok(idx) = v.parse::<isize>() {
                if idx > 0 { Some((idx as usize) - 1) } else { None }
            } else { None }
        } else { None }
    }).collect()
}

fn compute_bounds_xy(verts: &[Vec3]) -> Option<(f32,f32,f32,f32)> {
    if verts.is_empty() { return None; }
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (verts[0].x, verts[0].x, verts[0].y, verts[0].y);
    for v in verts.iter().skip(1) {
        if v.x < min_x { min_x = v.x; }
        if v.x > max_x { max_x = v.x; }
        if v.y < min_y { min_y = v.y; }
        if v.y > max_y { max_y = v.y; }
    }
    Some((min_x, max_x, min_y, max_y))
}

// Proyección ortográfica X/Y a la imagen (con padding y flip Y)
fn project_xy_to_image(x: f32, y: f32, bounds: (f32,f32,f32,f32), img_w: usize, img_h: usize, pad: f32) -> (f32,f32) {
    let (min_x, max_x, min_y, max_y) = bounds;
    let w = (max_x - min_x).max(1e-6);
    let h = (max_y - min_y).max(1e-6);
    let view_w = (img_w as f32) - 2.0*pad;
    let view_h = (img_h as f32) - 2.0*pad;
    let s = (view_w / w).min(view_h / h);
    let nx = (x - min_x)*s + pad + (view_w - w*s)*0.5;
    let ny = (max_y - y)*s + pad + (view_h - h*s)*0.5; // invierte Y
    (nx, ny)
}

/* ---------- Render del primer triángulo del OBJ ---------- */
fn render_first_triangle_from_obj(obj: &ObjData, out_path: &str) -> std::io::Result<()> {
    // buscar primera cara con >=3 vértices
    let mut tri: Option<[usize;3]> = None;
    for f in &obj.faces {
        let idxs = parse_face_vertex_indices(f);
        if idxs.len() >= 3 {
            tri = Some([idxs[0], idxs[1], idxs[2]]); // primer tri de la cara
            break;
        }
    }
    let tri = match tri {
        Some(t) => t,
        None => { eprintln!("No se encontró ninguna cara con 3 vértices."); return Ok(()); }
    };

    let bounds = match compute_bounds_xy(&obj.vertices) {
        Some(b) => b,
        None => { eprintln!("No hay vértices en el OBJ."); return Ok(()); }
    };

    let (w, h) = (800usize, 600usize);
    let mut img = Image::new(w, h);
    let pad = 40.0;

    let v0 = obj.vertices[tri[0]];
    let v1 = obj.vertices[tri[1]];
    let v2 = obj.vertices[tri[2]];

    let (x0, y0) = project_xy_to_image(v0.x, v0.y, bounds, w, h, pad);
    let (x1, y1) = project_xy_to_image(v1.x, v1.y, bounds, w, h, pad);
    let (x2, y2) = project_xy_to_image(v2.x, v2.y, bounds, w, h, pad);

    fill_triangle(&mut img, x0, y0, x1, y1, x2, y2, 30, 144, 255);
    save_bmp(out_path, w, h, &img.data)?;

    println!("[OK] Se dibujó el primer triángulo del OBJ en '{}'", out_path);
    println!("  Tri (v índices 1-based): {}, {}, {}", tri[0]+1, tri[1]+1, tri[2]+1);
    Ok(())
}

/* ---------- CLI ---------- */
fn print_usage() {
    eprintln!(r#"Usage:
  cargo run -- <obj_path> [--limit N] [--render-obj-triangle]

Options:
  --limit N              Trunca cada lista impresa (si no, imprime todo).
  --render-obj-triangle  Renderiza el primer triángulo del OBJ a 'obj_triangle.bmp'.
"#);
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let obj_path = PathBuf::from(&args[1]);
    let mut limit: Option<usize> = None;
    let mut render_obj_triangle = false;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--limit" => {
                if i+1 < args.len() {
                    if let Ok(v) = args[i+1].parse::<usize>() { limit = Some(v); }
                    i += 1;
                }
            },
            "--render-obj-triangle" => render_obj_triangle = true,
            _ => {}
        }
        i += 1;
    }

    let data = parse_obj(&obj_path)?;

    // Listas
    println!("Vectores:");
    if let Some(n) = limit {
        for v in data.vertices.iter().take(n) { println!("  ({}, {}, {})", v.x, v.y, v.z); }
        if data.vertices.len() > n { println!("  ... ({} más)", data.vertices.len()-n); }
    } else {
        for v in &data.vertices { println!("  ({}, {}, {})", v.x, v.y, v.z); }
    }

    println!("\nCoordenadas UV:");
    if let Some(n) = limit {
        for vt in data.uvs.iter().take(n) { println!("  ({}, {})", vt.x, vt.y); }
        if data.uvs.len() > n { println!("  ... ({} más)", data.uvs.len()-n); }
    } else {
        for vt in &data.uvs { println!("  ({}, {})", vt.x, vt.y); }
    }

    println!("\nNormales:");
    if let Some(n) = limit {
        for vn in data.normals.iter().take(n) { println!("  ({}, {}, {})", vn.x, vn.y, vn.z); }
        if data.normals.len() > n { println!("  ... ({} más)", data.normals.len()-n); }
    } else {
        for vn in &data.normals { println!("  ({}, {}, {})", vn.x, vn.y, vn.z); }
    }

    println!("\nCaras:");
    if let Some(n) = limit {
        for f in data.faces.iter().take(n) { println!("  {}", f); }
        if data.faces.len() > n { println!("  ... ({} más)", data.faces.len()-n); }
    } else {
        for f in &data.faces { println!("  {}", f); }
    }

    println!("\nMateriales:");
    if !data.used_materials.is_empty() {
        println!("  Usados (usemtl):");
        for m in &data.used_materials { println!("    {}", m); }
    } else { println!("  Usados (usemtl): —"); }
    if !data.mtllibs.is_empty() {
        println!("  Bibliotecas (mtllib):");
        for m in &data.mtllibs { println!("    {}", m); }
    } else { println!("  Bibliotecas (mtllib): —"); }

    if render_obj_triangle {
        render_first_triangle_from_obj(&data, "obj_triangle.bmp")?;
    }

    Ok(())
}
