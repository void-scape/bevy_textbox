//! Integrates `bevy_pretty_text` and `bevy_sequence` into a simple `TextBox`.
//!
//! Here is a simple example:
//! ```
//! fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
//!     commands.spawn(Camera2d);
//!
//!     let entity = commands
//!         .spawn((
//!             TextBox,
//!             Sprite {
//!                 image: asset_server.load("textbox.png"),
//!                 anchor: Anchor::TopLeft,
//!                 ..Default::default()
//!             },
//!             Transform::from_xyz(-600., 0., 0.),
//!         ))
//!         .with_child((
//!             Sprite {
//!                 image: asset_server.load("continue.png"),
//!                 anchor: Anchor::TopLeft,
//!                 ..Default::default()
//!             },
//!             Transform::from_translation(Vec3::default().with_z(100.)),
//!             Continue,
//!             Visibility::Hidden,
//!         ))
//!         .id();
//!
//!     let frag = (s!("`Hello|green`[0.5], `World`[wave]!"), "My name is Nic.")
//!         .always()
//!         .once()
//!         .on_end(move |mut commands: Commands| commands.entity(entity).despawn_recursive());
//!     spawn_root_with_context(frag, TextBoxEntity(entity), &mut commands);
//! }
//! ```

use bevy::prelude::*;
use bevy_pretty_text::prelude::*;
use bevy_sequence::{fragment::DataLeaf, prelude::*};
use std::sync::Arc;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct TextboxSystems;

pub struct TextboxPlugin;

impl Plugin for TextboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PrettyTextPlugin, SequencePlugin))
            .add_event::<UpdateContinueVis>()
            .add_event::<FragmentEvent<SectionFrag>>()
            .add_systems(
                Update,
                (spawn_section_frags, update_continue_visibility).in_set(TextboxSystems),
            );
    }
}

#[derive(Component)]
pub struct TextBox {
    bundle: TextBoxBundle,
}

impl TextBox {
    pub fn new(bundle: impl Bundle + Clone) -> Self {
        Self {
            bundle: TextBoxBundle::new(bundle),
        }
    }
}

#[derive(Clone)]
struct TextBoxBundle(Arc<dyn Fn(&mut EntityCommands) + Send + Sync + 'static>);

impl TextBoxBundle {
    fn new(bundle: impl Bundle + Clone) -> Self {
        Self(Arc::new(move |commands| {
            commands.insert(bundle.clone());
        }))
    }
}

#[derive(Component)]
pub struct TextBoxEntity(Entity);

impl TextBoxEntity {
    pub fn new(entity: Entity) -> Self {
        Self(entity)
    }
}

#[derive(Component)]
pub struct Continue;

#[derive(Event)]
pub struct UpdateContinueVis {
    entity: Entity,
    visibility: Visibility,
}

impl UpdateContinueVis {
    pub fn new(entity: Entity, visibility: Visibility) -> Self {
        Self { entity, visibility }
    }
}

fn update_continue_visibility(
    textbox_query: Query<&Children, With<TextBox>>,
    mut continue_query: Query<&mut Visibility, With<Continue>>,
    mut reader: EventReader<UpdateContinueVis>,
) {
    for event in reader.read() {
        if let Ok(children) = textbox_query.get(event.entity) {
            for child in children.iter() {
                if let Ok(mut cont) = continue_query.get_mut(child) {
                    *cont = event.visibility;
                }
            }
        }
    }
}

// TODO: this leaks memory (SystemId)
fn spawn_section_frags(
    mut commands: Commands,
    mut reader: EventReader<FragmentEvent<SectionFrag>>,
    textboxes: Query<&TextBox>,
) {
    for event in reader.read() {
        let textbox = event.data.textbox;
        let end = event.end();
        let entity = commands.spawn_empty().id();
        let on_end = commands.register_system(
            move |mut commands: Commands, mut writer: EventWriter<UpdateContinueVis>| {
                let id = commands.register_system(
                    move |mut commands: Commands,
                          mut frag_writer: EventWriter<FragmentEndEvent>,
                          mut continue_writer: EventWriter<UpdateContinueVis>| {
                        frag_writer.write(end);
                        commands.entity(entity).despawn();
                        continue_writer.write(UpdateContinueVis::new(textbox, Visibility::Hidden));
                    },
                );

                commands.entity(entity).insert((AwaitClear, OnClear(id)));
                writer.write(UpdateContinueVis::new(textbox, Visibility::Visible));
            },
        );

        let mut section = commands.entity(entity);
        let mut section_commands = section.insert((
            event.data.section.clone(),
            Scroll::default(),
            OnScrollEnd(on_end),
        ));
        (textboxes.get(textbox).unwrap().bundle.0)(&mut section_commands);
        let child = section_commands.id();
        commands.entity(textbox).add_child(child);
    }
}

#[derive(Clone)]
pub struct SectionFrag {
    textbox: Entity,
    section: TypeWriterSection,
}

macro_rules! impl_into_frag {
    ($ty:ty, $x:ident, $into:expr) => {
        impl IntoFragment<SectionFrag, TextBoxEntity> for $ty {
            fn into_fragment(
                self,
                context: &Context<TextBoxEntity>,
                commands: &mut Commands,
            ) -> FragmentId {
                let $x = self;
                <_ as IntoFragment<SectionFrag, TextBoxEntity>>::into_fragment(
                    DataLeaf::new(SectionFrag {
                        textbox: context.read().unwrap().0,
                        section: $into,
                    }),
                    context,
                    commands,
                )
            }
        }
    };
}

impl_into_frag!(&'static str, slf, slf.into());
impl_into_frag!(String, slf, slf.into());
impl_into_frag!(TypeWriterSection, slf, slf);
