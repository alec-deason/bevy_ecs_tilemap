use bevy::{
    core::FloatOrd,
    core_pipeline::{SetItemPipeline, Transparent2d},
    ecs::system::{
        lifetimeless::{Read, SQuery, SRes},
        SystemParamItem, SystemState,
    },
    math::Mat4,
    prelude::{
        Commands, Entity, FromWorld, GlobalTransform, Handle, HandleUntyped, Query, Res, ResMut,
        With, World,
    },
    reflect::TypeUuid,
    render2::{
        mesh::Mesh,
        render_asset::RenderAssets,
        render_component::{ComponentUniforms, DynamicUniformIndex},
        render_phase::{DrawFunctions, RenderCommand, RenderPhase, TrackedRenderPass},
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
            BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
            BlendComponent, BlendFactor, BlendOperation, BlendState, BufferBindingType, BufferSize,
            ColorTargetState, ColorWrites, Face, FragmentState, FrontFace, MultisampleState,
            PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipelineCache,
            RenderPipelineDescriptor, Shader, ShaderStages, SpecializedPipeline,
            SpecializedPipelines, TextureFormat, TextureSampleType, TextureViewDimension,
            VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, Image},
        view::{ExtractedView, Msaa, ViewUniformOffset, ViewUniforms},
    },
    utils::HashMap,
};
use crevice::std140::AsStd140;

use crate::Chunk;

use super::tilemap_data::TilemapUniformData;

pub const TILEMAP_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 8094008129742001941);

pub struct LayerId(u16);

pub fn extract_tilemaps(
    mut commands: Commands,
    query: Query<(
        Entity,
        &GlobalTransform,
        &Chunk,
        &TilemapUniformData,
        &Handle<Mesh>,
    )>,
) {
    let mut extracted_tilemaps = Vec::new();
    for (entity, transform, chunk, tilemap_uniform, mesh_handle) in query.iter() {
        let transform = transform.compute_matrix();
        extracted_tilemaps.push((
            entity,
            (
                LayerId(chunk.settings.layer_id),
                chunk.material.clone(),
                mesh_handle.clone_weak(),
                tilemap_uniform.clone(),
                MeshUniform { transform },
            ),
        ));
    }
    commands.insert_or_spawn_batch(extracted_tilemaps);
}

#[derive(Clone)]
pub struct TilemapPipeline {
    pub view_layout: BindGroupLayout,
    pub uniform_layout: BindGroupLayout,
    pub material_layout: BindGroupLayout,
    pub mesh_layout: BindGroupLayout,
}

#[derive(AsStd140, Clone)]
pub struct MeshUniform {
    pub transform: Mat4,
}

impl FromWorld for TilemapPipeline {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let view_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                // View
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        // TODO: change this to ViewUniform::std140_size_static once crevice fixes this!
                        // Context: https://github.com/LPGhatguy/crevice/issues/29
                        min_binding_size: BufferSize::new(144),
                    },
                    count: None,
                },
            ],
            label: Some("tilemap_view_layout"),
        });

        let mesh_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    // TODO: change this to MeshUniform::std140_size_static once crevice fixes this!
                    // Context: https://github.com/LPGhatguy/crevice/issues/29
                    min_binding_size: BufferSize::new(64),
                },
                count: None,
            }],
            label: Some("tilemap_mesh_layout"),
        });

        let uniform_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(56),
                },
                count: None,
            }],
            label: Some("tilemap_material_layout"),
        });

        let material_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler {
                        comparison: false,
                        filtering: true,
                    },
                    count: None,
                },
            ],
            label: Some("tilemap_material_layout"),
        });

        TilemapPipeline {
            view_layout,
            material_layout,
            mesh_layout,
            uniform_layout,
        }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    // NOTE: Apparently quadro drivers support up to 64x MSAA.
    /// MSAA uses the highest 6 bits for the MSAA sample count - 1 to support up to 64x MSAA.
    pub struct TilemapPipelineKey: u32 {
        const NONE               = 0;
        const MSAA_RESERVED_BITS = TilemapPipelineKey::MSAA_MASK_BITS << TilemapPipelineKey::MSAA_SHIFT_BITS;
    }
}

impl TilemapPipelineKey {
    const MSAA_MASK_BITS: u32 = 0b111111;
    const MSAA_SHIFT_BITS: u32 = 32 - 6;

    pub fn from_msaa_samples(msaa_samples: u32) -> Self {
        let msaa_bits = ((msaa_samples - 1) & Self::MSAA_MASK_BITS) << Self::MSAA_SHIFT_BITS;
        TilemapPipelineKey::from_bits(msaa_bits).unwrap()
    }

    pub fn msaa_samples(&self) -> u32 {
        ((self.bits >> Self::MSAA_SHIFT_BITS) & Self::MSAA_MASK_BITS) + 1
    }
}

impl SpecializedPipeline for TilemapPipeline {
    type Key = TilemapPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: TILEMAP_SHADER_HANDLE.typed::<Shader>(),
                entry_point: "vertex".into(),
                shader_defs: vec![],
                buffers: vec![VertexBufferLayout {
                    array_stride: 44,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        // Position (GOTCHA! Vertex_Position isn't first in the buffer due to how Mesh sorts attributes (alphabetically))
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: 16,
                            shader_location: 0,
                        },
                        // Uv
                        VertexAttribute {
                            format: VertexFormat::Sint32x4,
                            offset: 28,
                            shader_location: 1,
                        },
                        // Color
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            fragment: Some(FragmentState {
                shader: TILEMAP_SHADER_HANDLE.typed::<Shader>(),
                shader_defs: vec![],
                entry_point: "fragment".into(),
                targets: vec![ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                }],
            }),
            layout: Some(vec![
                self.view_layout.clone(),
                self.mesh_layout.clone(),
                self.uniform_layout.clone(),
                self.material_layout.clone(),
            ]),
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                clamp_depth: false,
                conservative: false,
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1, //key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("tilemap_pipeline".into()),
        }
    }
}

pub struct TransformBindGroup {
    pub value: BindGroup,
}

pub fn queue_transform_bind_group(
    mut commands: Commands,
    tilemap_pipeline: Res<TilemapPipeline>,
    render_device: Res<RenderDevice>,
    transform_uniforms: Res<ComponentUniforms<MeshUniform>>,
) {
    if let Some(binding) = transform_uniforms.uniforms().binding() {
        commands.insert_resource(TransformBindGroup {
            value: render_device.create_bind_group(&BindGroupDescriptor {
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: binding,
                }],
                label: Some("transform_bind_group"),
                layout: &tilemap_pipeline.mesh_layout,
            }),
        });
    }
}

pub struct TilemapUniformDataBindGroup {
    pub value: BindGroup,
}

pub fn queue_tilemap_bind_group(
    mut commands: Commands,
    tilemap_pipeline: Res<TilemapPipeline>,
    render_device: Res<RenderDevice>,
    tilemap_uniforms: Res<ComponentUniforms<TilemapUniformData>>,
) {
    if let Some(binding) = tilemap_uniforms.uniforms().binding() {
        commands.insert_resource(TilemapUniformDataBindGroup {
            value: render_device.create_bind_group(&BindGroupDescriptor {
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: binding,
                }],
                label: Some("tilemap_bind_group"),
                layout: &tilemap_pipeline.uniform_layout,
            }),
        });
    }
}

pub struct TilemapViewBindGroup {
    pub value: BindGroup,
}

#[derive(Default)]
pub struct ImageBindGroups {
    values: HashMap<Handle<Image>, BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub fn queue_meshes(
    mut commands: Commands,
    transparent_2d_draw_functions: Res<DrawFunctions<Transparent2d>>,
    render_device: Res<RenderDevice>,
    tilemap_pipeline: Res<TilemapPipeline>,
    mut pipelines: ResMut<SpecializedPipelines<TilemapPipeline>>,
    mut pipeline_cache: ResMut<RenderPipelineCache>,
    msaa: Res<Msaa>,
    view_uniforms: Res<ViewUniforms>,
    gpu_images: Res<RenderAssets<Image>>,
    mut image_bind_groups: ResMut<ImageBindGroups>,
    standard_tilemap_meshes: Query<
        (Entity, &LayerId, &Handle<Image>, &MeshUniform),
        With<Handle<Mesh>>,
    >,
    mut views: Query<(Entity, &ExtractedView, &mut RenderPhase<Transparent2d>)>,
) {
    if let Some(view_binding) = view_uniforms.uniforms.binding() {
        let msaa_key = TilemapPipelineKey::from_msaa_samples(msaa.samples);
        for (entity, _view, mut transparent_phase) in views.iter_mut() {
            let view_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: view_binding.clone(),
                }],
                label: Some("tilemap_view_bind_group"),
                layout: &tilemap_pipeline.view_layout,
            });

            commands.entity(entity).insert(TilemapViewBindGroup {
                value: view_bind_group,
            });

            let draw_tilemap = transparent_2d_draw_functions
                .read()
                .get_id::<DrawTilemap>()
                .unwrap();

            for (entity, layer_id, image, _mesh_uniform) in standard_tilemap_meshes.iter() {
                image_bind_groups
                    .values
                    .entry(image.clone_weak())
                    .or_insert_with(|| {
                        let gpu_image = gpu_images.get(&image).unwrap();
                        render_device.create_bind_group(&BindGroupDescriptor {
                            entries: &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::TextureView(&gpu_image.texture_view),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::Sampler(&gpu_image.sampler),
                                },
                            ],
                            label: Some("sprite_material_bind_group"),
                            layout: &tilemap_pipeline.material_layout,
                        })
                    });

                let pipeline_id =
                    pipelines.specialize(&mut pipeline_cache, &tilemap_pipeline, msaa_key);

                transparent_phase.add(Transparent2d {
                    entity,
                    draw_function: draw_tilemap,
                    pipeline: pipeline_id,
                    sort_key: FloatOrd(layer_id.0 as f32),
                });
            }
        }
    }
}

pub struct SetMeshViewBindGroup<const I: usize>;
impl<const I: usize> RenderCommand<Transparent2d> for SetMeshViewBindGroup<I> {
    type Param = SQuery<(Read<ViewUniformOffset>, Read<TilemapViewBindGroup>)>;
    #[inline]
    fn render<'w>(
        view: Entity,
        _item: &Transparent2d,
        view_query: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) {
        let (view_uniform, pbr_view_bind_group) = view_query.get(view).unwrap();
        pass.set_bind_group(I, &pbr_view_bind_group.value, &[view_uniform.offset]);
    }
}

pub struct SetTransformBindGroup<const I: usize>;
impl<const I: usize> RenderCommand<Transparent2d> for SetTransformBindGroup<I> {
    type Param = (
        SRes<TransformBindGroup>,
        SQuery<Read<DynamicUniformIndex<MeshUniform>>>,
    );
    #[inline]
    fn render<'w>(
        _view: Entity,
        item: &Transparent2d,
        (transform_bind_group, mesh_query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) {
        let transform_index = mesh_query.get(item.entity).unwrap();
        pass.set_bind_group(
            I,
            &transform_bind_group.into_inner().value,
            &[transform_index.index()],
        );
    }
}

pub struct SetTilemapBindGroup<const I: usize>;
impl<const I: usize> RenderCommand<Transparent2d> for SetTilemapBindGroup<I> {
    type Param = (
        SRes<TilemapUniformDataBindGroup>,
        SQuery<Read<DynamicUniformIndex<TilemapUniformData>>>,
    );
    #[inline]
    fn render<'w>(
        _view: Entity,
        item: &Transparent2d,
        (tilemap_bind_group, mesh_query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) {
        let tilemap_uniform_index = mesh_query.get(item.entity).unwrap();
        pass.set_bind_group(
            I,
            &tilemap_bind_group.into_inner().value,
            &[tilemap_uniform_index.index()],
        );
    }
}

pub struct SetMaterialBindGroup<const I: usize>;
impl<const I: usize> RenderCommand<Transparent2d> for SetMaterialBindGroup<I> {
    type Param = (SRes<ImageBindGroups>, SQuery<Read<Handle<Image>>>);
    #[inline]
    fn render<'w>(
        _view: Entity,
        item: &Transparent2d,
        (image_bind_groups, entities_with_images): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) {
        let image_handle = entities_with_images.get(item.entity).unwrap();
        let bind_group = image_bind_groups
            .into_inner()
            .values
            .get(image_handle)
            .unwrap();
        pass.set_bind_group(I, &bind_group, &[]);
    }
}

pub type DrawTilemap = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetTransformBindGroup<1>,
    SetTilemapBindGroup<2>,
    SetMaterialBindGroup<3>,
    DrawMesh,
);

pub struct DrawMesh;
impl RenderCommand<Transparent2d> for DrawMesh {
    type Param = (SRes<RenderAssets<Mesh>>, SQuery<Read<Handle<Mesh>>>);
    #[inline]
    fn render<'w>(
        _view: Entity,
        item: &Transparent2d,
        (meshes, mesh_query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) {
        let mesh_handle = mesh_query.get(item.entity).unwrap();
        let gpu_mesh = meshes.into_inner().get(mesh_handle).unwrap();
        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        if let Some(index_info) = &gpu_mesh.index_info {
            pass.set_index_buffer(index_info.buffer.slice(..), 0, index_info.index_format);
            pass.draw_indexed(0..index_info.count, 0, 0..1);
        } else {
            panic!("non-indexed drawing not supported yet")
        }
    }
}