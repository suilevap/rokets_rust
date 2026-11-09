use tilerider::assets; use tilerider::content::map::MapData; use tilerider::map::grid::MapGrid;
fn main(){
 let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
 let xml=std::fs::read_to_string(root.join(assets::map_path("LetItSnowDay").unwrap())).unwrap();
 let map=MapData::parse(&xml).unwrap();
 let f=std::fs::File::open(root.join(assets::sprite_path(map.image.as_deref().unwrap()).unwrap())).unwrap();
 let mut d=png::Decoder::new(std::io::BufReader::new(f)); d.set_transformations(png::Transformations::normalize_to_color8());
 let mut r=d.read_info().unwrap(); let mut buf=vec![0;r.output_buffer_size().unwrap()]; let info=r.next_frame(&mut buf).unwrap();
 let grid=MapGrid::build(&map, Some((info.width as usize, info.height as usize, &buf[..])));
 for (x,y,w,h) in grid.wall_rects() { if x<=44 && x+w>=36 && y<=27 && y+h>=22 { println!("rect {x},{y} {w}x{h}"); } }
 for y in 20..28 { for x in 34..46 { if let Some(e)=grid.edge(x,y) { println!("edge {x},{y} open {}", e.open);} } }
 for y in 22..28 { println!("{y}: {}", (34..46).map(|x| if grid.is_wall(x,y) {'#'} else {'.'}).collect::<String>()); }
}
