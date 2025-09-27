use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/* ============================================================================
   Tipos básicos para representar datos geométricos del OBJ
   ============================================================================ */

// Se representa un UV como par (u, v)
#[derive(Debug, Clone, Copy)]
struct Vec2 { x: f32, y: f32 }

// Se representa un vértice/normal como (x, y, z)
#[derive(Debug, Clone, Copy)]
struct Vec3 { x: f32, y: f32, z: f32 }

/* ============================================================================
   Estructura con todo lo que interesa del archivo .OBJ
   ============================================================================ */

#[derive(Debug)]
// Estructura para almacenar datos del OBJ
struct ObjData {
    vertices: Vec<Vec3>,        // líneas "v"
    uvs: Vec<Vec2>,             // líneas "vt"
    normals: Vec<Vec3>,         // líneas "vn"
    faces: Vec<String>,         // líneas "f" crudas, del tipo "v/vt/vn v/vt/vn ..."
    mtllibs: Vec<String>,       // líneas "mtllib ..." (nombres de .mtl referenciados)
    used_materials: Vec<String>,// líneas "usemtl ..." (material en uso para las caras siguientes)
}

/* ============================================================================
   Parser de un archivo .OBJ
   - Se lee el archivo línea por línea
   - Se identifica el "tag" (v, vt, vn, f, mtllib, usemtl, ...)
   - Se acumula lo relevante en la estructura ObjData
   ============================================================================ */

fn parse_obj(path: &PathBuf) -> std::io::Result<ObjData> {
    // Se abre el archivo y se prepara un BufReader para lectura eficiente línea a línea
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Se preparan los buffers donde se guardará lo encontrado
    let mut vertices = Vec::<Vec3>::new();
    let mut uvs = Vec::<Vec2>::new();
    let mut normals = Vec::<Vec3>::new();
    let mut faces = Vec::<String>::new();
    let mut mtllibs = Vec::<String>::new();
    let mut used_materials = Vec::<String>::new();

    // Se recorre cada línea del archivo
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // Se ignoran líneas vacías y comentarios (empiezan con '#')
        if line.is_empty() || line.starts_with('#') { continue; }

        // Se separa por espacios: el primer token es el "tag"
        let mut it = line.split_whitespace();
        if let Some(tag) = it.next() {
            match tag {
                // Vértice: "v x y z"
                "v" => {
                    // Se parsean los floats (si hay más de 3, se toman los 3 primeros)
                    let vals: Vec<f32> = it.filter_map(|s| s.parse::<f32>().ok()).collect();
                    if vals.len() >= 3 { vertices.push(Vec3 { x: vals[0], y: vals[1], z: vals[2] }); }
                },

                // UV: "vt u v"
                "vt" => {
                    let vals: Vec<f32> = it.filter_map(|s| s.parse::<f32>().ok()).collect();
                    if vals.len() >= 2 { uvs.push(Vec2 { x: vals[0], y: vals[1] }); }
                },

                // Normal: "vn x y z"
                "vn" => {
                    let vals: Vec<f32> = it.filter_map(|s| s.parse::<f32>().ok()).collect();
                    if vals.len() >= 3 { normals.push(Vec3 { x: vals[0], y: vals[1], z: vals[2] }); }
                },

                // Cara: se guarda el resto tal cual, para parsearlo luego (v/vt/vn ...)
                "f" => faces.push(it.collect::<Vec<&str>>().join(" ")),

                // Bibliotecas de materiales referenciadas
                "mtllib" => mtllibs.push(it.collect::<Vec<&str>>().join(" ")),

                // Material en uso para las siguientes caras
                "usemtl" => used_materials.push(it.collect::<Vec<&str>>().join(" ")),

                // Cualquier otro tag se ignora en esta implementación minimal
                _ => {}
            }
        }
    }

    Ok(ObjData { vertices, uvs, normals, faces, mtllibs, used_materials })
}

/* ============================================================================
   Escritura de BMP de 24 bits "a mano"
   - pixels: buffer RGB (no BGR) con origen en la esquina superior izquierda
   - BMP exige filas alineadas a múltiplos de 4 bytes y formato BGR en disco
   - Se escriben headers BMP y DIB (BITMAPINFOHEADER) y luego las filas con padding
   ============================================================================ */

fn save_bmp(path: &str, width: usize, height: usize, pixels: &Vec<u8>) -> std::io::Result<()> {
    let mut f = File::create(path)?;

    // Cada píxel son 3 bytes (RGB). BMP exige filas alineadas a 4 bytes.
    let row_stride_unpadded = width * 3;
    let padding = (4 - (row_stride_unpadded as u32 % 4)) % 4;
    let row_stride = row_stride_unpadded as u32 + padding;

    // Cálculos de tamaños para los headers
    let pixel_array_size = row_stride * height as u32;
    let file_size = 14 + 40 + pixel_array_size; // 14 (BMP) + 40 (DIB) + píxeles

    // --- BMP HEADER (14 bytes) ---
    f.write_all(&[0x42, 0x4D])?;                 // 'BM'
    f.write_all(&file_size.to_le_bytes())?;      // tamaño total de archivo
    f.write_all(&[0u8; 4])?;                     // reservado
    f.write_all(&(14 + 40 as i32 as u32).to_le_bytes())?; // offset a datos de imagen

    // --- DIB HEADER: BITMAPINFOHEADER (40 bytes) ---
    f.write_all(&40u32.to_le_bytes())?;          // tamaño del DIB
    f.write_all(&(width as i32).to_le_bytes())?; // ancho
    // Alto negativo => top-down (coincide con el buffer con origen arriba)
    f.write_all(&(-(height as i32)).to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;           // planos
    f.write_all(&24u16.to_le_bytes())?;          // 24 bpp
    f.write_all(&0u32.to_le_bytes())?;           // sin compresión
    f.write_all(&pixel_array_size.to_le_bytes())?; // tamaño de la imagen
    f.write_all(&[0u8; 16])?;                    // resolución/paleta (no utilizada)

    // --- Datos de píxeles: se escribe BGR + padding por fila ---
    let mut row = vec![0u8; row_stride as usize];
    for y in 0..height {
        let start = y * row_stride_unpadded;
        for x in 0..width {
            let i = start + x*3;
            // Conversión de RGB (en memoria) a BGR (en disco BMP)
            row[x*3 + 0] = pixels[i+2]; // B
            row[x*3 + 1] = pixels[i+1]; // G
            row[x*3 + 2] = pixels[i+0]; // R
        }
        // Se añade padding hasta múltiplo de 4
        for p in (row_stride_unpadded)..(row_stride as usize) { row[p] = 0; }
        f.write_all(&row)?;
    }
    Ok(())
}

/* ============================================================================
   Framebuffer simple en RAM y raster de triángulo por "edge functions"
   ============================================================================ */

struct Image { w: usize, h: usize, data: Vec<u8> }

impl Image {
    // Se crea un lienzo blanco (RGB=255)
    fn new(w: usize, h: usize) -> Self { Self { w, h, data: vec![255; w*h*3] } }

    // Se escribe un píxel si cae dentro del lienzo
    fn set(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 { return; }
        let idx = ((y as usize) * self.w + (x as usize)) * 3;
        self.data[idx] = r; self.data[idx+1] = g; self.data[idx+2] = b;
    }
}

// Se define la edge function: área firmada del triángulo (a,b,p)
// Se utiliza para comprobar si un punto P se encuentra del mismo lado de los tres bordes
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax)*(by - ay) - (py - ay)*(bx - ax)
}

// Se realiza el rellenado de triángulo mediante coherencia de signos de las edge functions
fn fill_triangle(img: &mut Image, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, r: u8, g: u8, b: u8) {
    // Se calcula el AABB del triángulo para acotar el barrido
    let min_x = x0.min(x1).min(x2).floor().max(0.0) as i32;
    let max_x = x0.max(x1).max(x2).ceil().min((img.w-1) as f32) as i32;
    let min_y = y0.min(y1).min(y2).floor().max(0.0) as i32;
    let max_y = y0.max(y1).max(y2).ceil().min((img.h-1) as f32) as i32;

    // Si el área es 0, se considera degenerado (colineal) y no se dibuja
    let area = edge(x0,y0,x1,y1,x2,y2);
    if area == 0.0 { return; }

    // Se recorre el AABB y se testea si cada píxel cae dentro del triángulo
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5; // muestreo en el centro del píxel
            let py = y as f32 + 0.5;
            let w0 = edge(x1,y1,x2,y2,px,py);
            let w1 = edge(x2,y2,x0,y0,px,py);
            let w2 = edge(x0,y0,x1,y1,px,py);

            // Se acepta si los tres valores comparten signo (>=0 o <=0)
            if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                img.set(x,y,r,g,b);
            }
        }
    }
}

/* ============================================================================
   Utilidades:
   - Se parsean índices "v/vt/vn" de una cara y se devuelven solo índices v (0-based)
   - Se calcula el bounding box XY de todos los vértices
   - Se proyecta ortográficamente XY -> pantalla con padding y flip en Y
   ============================================================================ */

// Se devuelven índices de vértice (v) en 0-based a partir de tokens "v/vt/vn"
// Nota: para índices negativos se podría adaptar a "len + idx"
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

// Se calcula el bounding box 2D en el plano XY para escalar la vista
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

// Se proyecta ortográficamente (x,y) a coordenadas de imagen, con padding y flip en Y
fn project_xy_to_image(x: f32, y: f32, bounds: (f32,f32,f32,f32), img_w: usize, img_h: usize, pad: f32) -> (f32,f32) {
    let (min_x, max_x, min_y, max_y) = bounds;
    let w = (max_x - min_x).max(1e-6);
    let h = (max_y - min_y).max(1e-6);

    // Se define el área visible dentro de la imagen después del padding
    let view_w = (img_w as f32) - 2.0*pad;
    let view_h = (img_h as f32) - 2.0*pad;

    // Se calcula una escala uniforme para mantener aspecto
    let s = (view_w / w).min(view_h / h);

    // Se aplica traslado y escala; además se invierte Y para imagen (origen arriba)
    let nx = (x - min_x)*s + pad + (view_w - w*s)*0.5;
    let ny = (max_y - y)*s + pad + (view_h - h*s)*0.5;
    (nx, ny)
}

/* ============================================================================
   Render del PRIMER triángulo del OBJ:
   - Se busca la primera cara con >= 3 vértices
   - Se toman los primeros 3 (si es quad, se usa v0,v1,v2)
   - Se proyecta a imagen y se rasteriza
   ============================================================================ */

fn render_first_triangle_from_obj(obj: &ObjData, out_path: &str) -> std::io::Result<()> {
    // Se busca la primera cara con al menos 3 vértices
    let mut tri: Option<[usize;3]> = None;
    for f in &obj.faces {
        let idxs = parse_face_vertex_indices(f);
        if idxs.len() >= 3 {
            tri = Some([idxs[0], idxs[1], idxs[2]]); // primer tri de la cara
            break;
        }
    }

    // Si no se encuentra ninguna cara válida, se informa y se termina
    let tri = match tri {
        Some(t) => t,
        None => { eprintln!("No se encontró ninguna cara con 3 vértices."); return Ok(()); }
    };

    // Se obtiene el bounding box XY para escalar y centrar en pantalla
    let bounds = match compute_bounds_xy(&obj.vertices) {
        Some(b) => b,
        None => { eprintln!("No hay vértices en el OBJ."); return Ok(()); }
    };

    // Se crea un lienzo y un padding agradables
    let (w, h) = (800usize, 600usize);
    let mut img = Image::new(w, h);
    let pad = 40.0;

    // Se recuperan las posiciones de los 3 vértices del tri en 3D
    let v0 = obj.vertices[tri[0]];
    let v1 = obj.vertices[tri[1]];
    let v2 = obj.vertices[tri[2]];

    // Se proyectan (x,y) a coordenadas de imagen
    let (x0, y0) = project_xy_to_image(v0.x, v0.y, bounds, w, h, pad);
    let (x1, y1) = project_xy_to_image(v1.x, v1.y, bounds, w, h, pad);
    let (x2, y2) = project_xy_to_image(v2.x, v2.y, bounds, w, h, pad);

    // Se rellena el triángulo con un color celeste
    fill_triangle(&mut img, x0, y0, x1, y1, x2, y2, 30, 144, 255);

    // Se guarda el resultado a disco
    save_bmp(out_path, w, h, &img.data)?;

    // Se muestra información en consola
    println!("[OK] Se dibujó el primer triángulo del OBJ en '{}'", out_path);
    println!("  Tri (v índices 1-based): {}, {}, {}", tri[0]+1, tri[1]+1, tri[2]+1);
    Ok(())
}

/* ============================================================================
   CLI:
   - cargo run -- <obj_path> [--limit N] [--render-obj-triangle]
   - Se imprimen listas (v, vt, vn, f, materiales) y opcionalmente se renderiza el 1er tri
   ============================================================================ */

fn print_usage() {
    eprintln!(r#"Usage:
  cargo run -- <obj_path> [--limit N] [--render-obj-triangle]

Options:
  --limit N              Trunca cada lista impresa (si no, se imprime todo).
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

    // Se parsean flags opcionales
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--limit" => {
                if i+1 < args.len() {
                    if let Ok(v) = args[i+1].parse::<usize>() { limit = Some(v); }
                    i += 1; // se consume el número
                }
            },
            "--render-obj-triangle" => render_obj_triangle = true,
            _ => {}
        }
        i += 1;
    }

    // Se parsea el OBJ a la estructura interna
    let data = parse_obj(&obj_path)?;

    // --- Se imprimen las listas en el formato requerido ---
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

    // Se renderiza el primer triángulo encontrado en el OBJ (opcional)
    if render_obj_triangle {
        render_first_triangle_from_obj(&data, "obj_triangle.bmp")?;
    }

    Ok(())
}
