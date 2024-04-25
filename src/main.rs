use pollster::FutureExt;
use r3d::{
    event::{event_types, EventHandler},
    gfx::{
        BindGroupEntryResource, BindingPropKey, Camera, CameraClearMode,
        CameraPerspectiveProjection, CameraPerspectiveProjectionAspect, CameraProjection, Color,
        Material, MaterialHandle, Mesh, MeshHandle, MeshRenderer, ShaderHandle, Texture,
    },
    image,
    input::InputDevice,
    math::{Mat4, Quat, Vec3},
    object::ObjectHandle,
    russimp::{self, node::Node},
    specs::Builder,
    transform::{Transform, TransformComponent},
    use_context,
    wgpu::{Device, TextureFormat},
    ContextHandle, Engine, EngineConfig, EngineExecError, EngineInitError, EngineLoopMode,
    EngineTargetFps,
};
use std::{collections::HashMap, path::Path};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("engine init error: {0}")]
    EngineInitError(#[from] EngineInitError),
    #[error("engine exec error: {0}")]
    EngineExecError(#[from] EngineExecError),
}

fn main() -> Result<(), Error> {
    let engine = Engine::new(EngineConfig {
        title: format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        resizable: false,
        width: 800,
        height: 600,
    })
    .block_on()?;

    init(engine.context());

    engine.run(EngineLoopMode::Poll, EngineTargetFps::VSync)?;
    Ok(())
}

fn init(ctx: ContextHandle) {
    let eye = Vec3::new(0.75, 1.0, 2.0);
    let target = Vec3::new(0.0, 1.0, 0.0);
    let look_at = Mat4::look_at(eye, target, Vec3::UP);

    let camera = Camera::new(
        0xFFFF_FFFF,
        0,
        CameraClearMode::All {
            color: Color::parse_hex("#03a1fc").unwrap(),
            depth: 1.0,
            stencil: 0,
        },
        CameraProjection::Perspective(CameraPerspectiveProjection {
            fov: 60.0f32.to_radians(),
            aspect: CameraPerspectiveProjectionAspect::Screen,
            near: 0.1,
            far: 1000.0,
        }),
        &ctx.gfx_ctx().device,
        ctx.render_mgr_mut().bind_group_layout_cache(),
    );
    {
        let mut world = ctx.world_mut();
        let mut object_mgr = ctx.object_mgr_mut();
        let (camera_object_id, builder) = object_mgr.create_object_builder(
            &mut world,
            Some("camera".to_owned()),
            Some(Transform::from_mat4(&look_at)),
        );
        builder.with(camera).build();
        camera_object_id
    };

    let shader = ctx
            .shader_mgr()
            .create_shader(
                ctx.render_mgr_mut().bind_group_layout_cache(),
                "
@group(0) @binding(0) var<uniform> camera_transform: mat4x4<f32>;
@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;

struct InstanceInput {
    @location(0) transform_row_0: vec4<f32>,
    @location(1) transform_row_1: vec4<f32>,
    @location(2) transform_row_2: vec4<f32>,
    @location(3) transform_row_3: vec4<f32>,
};

struct VertexInput {
    @location(4) position: vec3<f32>,
    @location(5) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(instance: InstanceInput, vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let transform = mat4x4<f32>(instance.transform_row_0, instance.transform_row_1, instance.transform_row_2, instance.transform_row_3);
    out.position = vec4<f32>(camera_transform * transform * vec4<f32>(vertex.position, 1.0));
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = textureSample(texture, texture_sampler, in.uv);
    return out;
}",
            )
            .unwrap();
    let mut materials = HashMap::new();

    fn create_textured_material(
        ctx: &ContextHandle,
        shader: &ShaderHandle,
        path: impl AsRef<Path>,
    ) -> MaterialHandle {
        let mut render_mgr = ctx.render_mgr_mut();
        let mut material = Material::new(shader.clone(), render_mgr.pipeline_layout_cache());
        let texture = Texture::from_image(
            TextureFormat::Rgba8UnormSrgb,
            &image::open(path).unwrap().flipv(),
            &ctx.gfx_ctx().device,
            &ctx.gfx_ctx().queue,
        );
        material.set_bind_property(
            &BindingPropKey::StringKey("texture".to_owned()),
            BindGroupEntryResource::TextureView {
                texture_view: texture.view.clone(),
            },
        );
        material.set_bind_property(
            &BindingPropKey::StringKey("texture_sampler".to_owned()),
            BindGroupEntryResource::Sampler {
                sampler: texture.sampler.clone(),
            },
        );
        material.update_bind_group(&ctx.gfx_ctx().device);
        MaterialHandle::new(material)
    }

    materials.insert(
        "Body",
        create_textured_material(
            &ctx,
            &shader,
            // "/Users/ashrimp/Downloads/Karin Body&Face Textures/Karin_Face_Tex.png",
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Face.png",
        ),
    );
    materials.insert(
        "body_2",
        create_textured_material(
            &ctx,
            &shader,
            // "/Users/ashrimp/Downloads/Karin Body&Face Textures/Karin_Body_Tex.png",
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Body.png",
        ),
    );
    materials.insert(
        "knee-socks",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Costume.png",
        ),
    );
    materials.insert(
        "hair",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Hair.png",
        ),
    );
    materials.insert(
        "kemomimi",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Hair.png",
        ),
    );
    materials.insert(
        "tail",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Hair.png",
        ),
    );
    materials.insert(
        "pullover",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Costume.png",
        ),
    );
    materials.insert(
        "shoes",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Costume.png",
        ),
    );
    materials.insert(
        "skirt",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Costume.png",
        ),
    );
    materials.insert(
        "underwear",
        create_textured_material(
            &ctx,
            &shader,
            "/Users/ashrimp/Downloads/Karin_v1.11/Textures/Karin_Costume.png",
        ),
    );

    let scene = {
        let file =
            std::fs::read("/Users/ashrimp/Downloads/Karin_v1.11/FBX/Karin_ver1.1.1.fbx").unwrap();
        russimp::scene::Scene::from_buffer(
            &file,
            vec![
                russimp::scene::PostProcess::JoinIdenticalVertices,
                russimp::scene::PostProcess::Triangulate,
                russimp::scene::PostProcess::SortByPrimitiveType,
                russimp::scene::PostProcess::SplitLargeMeshes,
                russimp::scene::PostProcess::ImproveCacheLocality,
            ],
            "",
        )
        .unwrap()
    };
    let meshes = HashMap::from_iter(
        scene
            .meshes
            .into_iter()
            .enumerate()
            .map(|(index, mesh)| (index as u32, MeshHandle::new(Mesh { data: mesh }))),
    );

    fn deploy_parts(
        device: &Device,
        materials: &HashMap<&str, MaterialHandle>,
        meshes: &HashMap<u32, MeshHandle>,
        node: &Node,
    ) -> ObjectHandle {
        let children = Vec::from_iter(
            node.children
                .borrow()
                .iter()
                .map(|child| deploy_parts(device, materials, meshes, child)),
        );

        let matrix = &node.transformation;
        let matrix = Mat4::new([
            matrix.a1, matrix.b1, matrix.c1, matrix.d1, matrix.a2, matrix.b2, matrix.c2, matrix.d2,
            matrix.a3, matrix.b3, matrix.c3, matrix.d3, matrix.a4, matrix.b4, matrix.c4, matrix.d4,
        ]);

        let object = if node.meshes.len() == 1 {
            let mut mesh_renderer = MeshRenderer::new();
            mesh_renderer.set_material(materials.get(node.name.as_str()).unwrap().clone());
            mesh_renderer.set_mesh(meshes.get(&node.meshes[0]).unwrap().clone(), device);

            let transform = Transform::from_mat4(&matrix);

            println!("{}", node.name);

            let mut world = use_context().world_mut();
            let mut object_mgr = use_context().object_mgr_mut();
            let (object, builder) = object_mgr.create_object_builder(
                &mut world,
                Some(node.name.to_owned()),
                Some(transform),
            );
            builder.with(mesh_renderer).build();

            object
        } else {
            let transform = Transform::from_mat4(&matrix);

            let object = {
                let mut world = use_context().world_mut();
                let mut object_mgr = use_context().object_mgr_mut();
                let (object, builder) = object_mgr.create_object_builder(
                    &mut world,
                    Some(node.name.to_owned()),
                    Some(transform),
                );
                builder.build();
                object
            };

            for (index, &mesh) in node.meshes.iter().enumerate() {
                println!("{}", node.name);

                let mut mesh_renderer = MeshRenderer::new();
                mesh_renderer.set_material(materials.get(node.name.as_str()).unwrap().clone());
                mesh_renderer.set_mesh(meshes.get(&mesh).unwrap().clone(), device);

                let mesh_object = {
                    let mut world = use_context().world_mut();
                    let mut object_mgr = use_context().object_mgr_mut();
                    let (mesh_object, builder) = object_mgr.create_object_builder(
                        &mut world,
                        Some(format!("{}-mesh-{}", node.name, index)),
                        None,
                    );
                    builder.with(mesh_renderer).build();
                    mesh_object
                };

                mesh_object.set_parent(&object);
            }

            object
        };

        for child in children {
            child.set_parent(&object);
        }

        object
    }

    deploy_parts(
        &ctx.gfx_ctx().device,
        &materials,
        &meshes,
        &scene.root.unwrap(),
    );

    ctx.event_mgr()
        .add_handler(EventHandler::<event_types::Update>::new(|_| update()));
}

fn update() {
    let ctx = use_context();
    let delta = ctx.time_mgr().delta_time().as_secs_f32();
    let input_mgr = ctx.input_mgr();
    let keyboard = input_mgr.keyboard();

    let z = keyboard.input("w").unwrap().value - keyboard.input("s").unwrap().value;
    let x = keyboard.input("d").unwrap().value - keyboard.input("a").unwrap().value;

    let vertical = keyboard.input("up").unwrap().value - keyboard.input("down").unwrap().value;
    let horizontal = keyboard.input("left").unwrap().value - keyboard.input("right").unwrap().value;

    let camera_object = ctx.object_mgr().find("camera").unwrap();
    let camera_transform = camera_object.component::<TransformComponent>();

    let forward = camera_transform.forward();
    let right = camera_transform.right();
    let world_to_local_rotation = camera_transform.world_rotation().conjugated();
    let world_up = Vec3::UP * world_to_local_rotation;

    camera_transform.set_position(camera_transform.position() + delta * z * forward);
    camera_transform.set_position(camera_transform.position() + delta * x * right);
    camera_transform.set_rotation(
        camera_transform.rotation()
            * Quat::from_axis_angle(Vec3::RIGHT, delta * vertical * 120.0f32.to_radians()),
    );
    camera_transform.set_rotation(
        camera_transform.rotation()
            * Quat::from_axis_angle(world_up, delta * horizontal * 120.0f32.to_radians()),
    );
}
