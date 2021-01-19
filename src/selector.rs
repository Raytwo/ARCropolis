// #[derive(ramhorns::Content)]
// pub struct Workspaces {
//     pub workspace: Vec<Workspace>
// }

// #[derive(ramhorns::Content)]
// pub struct Workspace {
//     pub index: u8,
//     pub name: String,
//     pub in_use: bool,
// }


// let mut file = std::fs::File::open("sd:/selector/index.html").unwrap();
    // let mut page_content: String = String::new();
    // file.read_to_string(&mut page_content);

    // let tpl = ramhorns::Template::new(page_content).unwrap();

    // let workspaces = Workspaces {
    //     workspace: vec![
    //     Workspace {
    //         index: 0,
    //         name: String::from("Mowjoh"),
    //         in_use: false,
    //     },
    //     Workspace {
    //         index: 1,
    //         name: String::from("Raytwo"),
    //         in_use: true,
    //     },
    //     Workspace {
    //         index: 2,
    //         name: String::from("DSX8"),
    //         in_use: false,
    //     },
    // ],
    // };

    // let render = tpl.render(&workspaces);

    // let mut webpage = skyline_web::Webpage::new();
    // webpage.htdocs_dir("selector");
    // webpage.file("index.html", &render);
    // webpage.open();