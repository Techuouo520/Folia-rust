// Checked Steam Audio boundary; every native handle and audio buffer is released before its library.

use std::collections::HashMap;
use std::path::Path;
use std::ptr;

use folia_core::{RoomImpulseResponse, RoomListener, RoomOutput, RoomRequest};

use crate::impulse::{SpeakerImpulse, assemble};
use crate::{Error, ffi};

const FRAME: usize = 1024;
const ORDER: i32 = 3;
const CHANNELS: i32 = 16;
const RAYS: i32 = 32768;

pub struct Simulator {
    api: ffi::Phonon,
    context: ffi::IPLContext,
    embree: ffi::IPLEmbreeDevice,
    hrtfs: HashMap<u32, ffi::IPLHRTF>,
}

impl Simulator {
    pub fn open(path: &Path) -> Result<Self, Error> {
        // SAFETY: The pinned SDK defines these generated signatures, and Phonon retains the loaded library.
        let api = unsafe { ffi::Phonon::new(path) }?;
        let mut settings =
            ffi::IPLContextSettings { version: (4 << 16) | (8 << 8) | 1, simdLevel: 4, ..Default::default() };
        // SAFETY: Settings and output pointer are valid for this synchronous creation call.
        let context = create("context", api.iplContextRelease, |handle| unsafe {
            (api.iplContextCreate)(&mut settings, handle)
        })?;
        let mut result = Self { api, context, embree: ptr::null_mut(), hrtfs: HashMap::new() };
        // SAFETY: A live context is retained, and this SDK's reserved Embree settings are null.
        let embree = create("Embree device", result.api.iplEmbreeDeviceRelease, |handle| unsafe {
            (result.api.iplEmbreeDeviceCreate)(result.context, ptr::null_mut(), handle)
        });
        match embree {
            Ok(device) => result.embree = device,
            Err(error) => {
                tracing::warn!(error = &error as &dyn std::error::Error, "Steam Audio using its built-in ray tracer")
            }
        }
        Ok(result)
    }

    pub fn compute(&mut self, request: &RoomRequest) -> Result<RoomImpulseResponse, Error> {
        validate(request)?;
        let rate = request.sample_rate;
        let mut audio = ffi::IPLAudioSettings { samplingRate: rate as i32, frameSize: FRAME as i32 };
        if !self.hrtfs.contains_key(&rate) {
            let mut settings = ffi::IPLHRTFSettings { volume: 1.0, ..Default::default() };
            // SAFETY: Both settings and the context outlive the native HRTF creation.
            let hrtf = create("HRTF", self.api.iplHRTFRelease, |handle| unsafe {
                (self.api.iplHRTFCreate)(self.context, &mut audio, &mut settings, handle)
            })?;
            self.hrtfs.insert(rate, hrtf);
        }
        let hrtf = self.hrtfs[&rate];
        let mut settings = ffi::IPLSceneSettings {
            type_: i32::from(!self.embree.is_null()),
            embreeDevice: self.embree,
            ..Default::default()
        };
        // SAFETY: The context and optional Embree device stay alive until all request objects are dropped.
        let scene = Handle::new(&self.api, "scene", self.api.iplSceneRelease, |handle| unsafe {
            (self.api.iplSceneCreate)(self.context, &mut settings, handle)
        })?;
        let meshes = self.meshes(request, scene.raw)?;
        // SAFETY: Every mesh was validated, copied by the SDK and is held for this request.
        unsafe { (self.api.iplSceneCommit)(scene.raw) };
        let simulator = self.simulator(&audio, scene.raw)?;
        let sources = self.sources(request, simulator.raw);
        let sources = sources?;
        let mut shared = ffi::IPLSimulationSharedInputs {
            listener: space(request.listener),
            numRays: RAYS,
            numBounces: 128,
            duration: 2.0,
            order: ORDER,
            irradianceMinDistance: 1.0,
            ..Default::default()
        };
        // SAFETY: Scene, sources, meshes and listener remain live for the complete synchronous simulation.
        unsafe {
            (self.api.iplSimulatorSetSharedInputs)(simulator.raw, 3, &mut shared);
            (self.api.iplSimulatorCommit)(simulator.raw);
            (self.api.iplSimulatorRunDirect)(simulator.raw);
            (self.api.iplSimulatorRunReflections)(simulator.raw);
        }
        let mut effects = Effects::new(&self.api, self.context, &mut audio, hrtf, request)?;
        let mut impulses = Vec::with_capacity(2);
        for (source, position) in sources.iter().zip(request.speakers) {
            let mut outputs = ffi::IPLSimulationOutputs::default();
            // SAFETY: This source belongs to the completed simulator and outputs is writable SDK-sized storage.
            unsafe { (self.api.iplSourceGetOutputs)(source.raw, 3, &mut outputs) };
            impulses.push(effects.speaker(outputs, position, request));
        }
        let result = assemble(&impulses, rate);
        drop(effects);
        drop(sources);
        drop(simulator);
        drop(meshes);
        drop(scene);
        Ok(result)
    }

    fn meshes<'a>(
        &'a self,
        request: &RoomRequest,
        scene: ffi::IPLScene,
    ) -> Result<Vec<Handle<'a, ffi::_IPLStaticMesh_t>>, Error> {
        let mut materials: Vec<_> = request
            .world
            .materials
            .iter()
            .map(|material| ffi::IPLMaterial {
                absorption: material.absorption,
                scattering: material.scattering,
                transmission: material.transmission,
            })
            .collect();
        request
            .world
            .meshes
            .iter()
            .map(|mesh| {
                let mut vertices: Vec<_> = mesh.positions.iter().copied().map(vector).collect();
                let mut triangles: Vec<_> = mesh
                    .triangles
                    .iter()
                    .map(|triangle| ffi::IPLTriangle { indices: triangle.map(|index| index as i32) })
                    .collect();
                let mut indices: Vec<i32> = mesh.triangle_materials.iter().copied().map(i32::from).collect();
                let mut settings = ffi::IPLStaticMeshSettings {
                    numVertices: vertices.len() as i32,
                    numTriangles: triangles.len() as i32,
                    numMaterials: materials.len() as i32,
                    vertices: vertices.as_mut_ptr(),
                    triangles: triangles.as_mut_ptr(),
                    materialIndices: indices.as_mut_ptr(),
                    materials: materials.as_mut_ptr(),
                };
                // SAFETY: Validation bounds every triangle and material index; SDK copies these arrays during creation.
                let handle = Handle::new(&self.api, "mesh", self.api.iplStaticMeshRelease, |handle| unsafe {
                    (self.api.iplStaticMeshCreate)(scene, &mut settings, handle)
                })?;
                // SAFETY: Mesh and scene belong to the same retained context.
                unsafe { (self.api.iplStaticMeshAdd)(handle.raw, scene) };
                Ok(handle)
            })
            .collect()
    }

    fn simulator<'a>(
        &'a self,
        audio: &ffi::IPLAudioSettings,
        scene: ffi::IPLScene,
    ) -> Result<Handle<'a, ffi::_IPLSimulator_t>, Error> {
        let threads =
            std::thread::available_parallelism()?.get().saturating_sub(1).max(1).min(i32::MAX as usize) as i32;
        let mut settings = ffi::IPLSimulationSettings {
            flags: 3,
            sceneType: i32::from(!self.embree.is_null()),
            reflectionType: 0,
            maxNumOcclusionSamples: 16,
            maxNumRays: RAYS,
            numDiffuseSamples: 32,
            maxDuration: 2.0,
            maxOrder: ORDER,
            maxNumSources: 2,
            numThreads: threads,
            rayBatchSize: 1,
            samplingRate: audio.samplingRate,
            frameSize: audio.frameSize,
            ..Default::default()
        };
        // SAFETY: Settings are initialized and this context remains alive for the returned simulator.
        let simulator = Handle::new(&self.api, "simulator", self.api.iplSimulatorRelease, |handle| unsafe {
            (self.api.iplSimulatorCreate)(self.context, &mut settings, handle)
        })?;
        // SAFETY: Scene is live and held by compute until after simulator disposal.
        unsafe { (self.api.iplSimulatorSetScene)(simulator.raw, scene) };
        Ok(simulator)
    }

    fn sources<'a>(
        &'a self,
        request: &RoomRequest,
        simulator: ffi::IPLSimulator,
    ) -> Result<Vec<Handle<'a, ffi::_IPLSource_t>>, Error> {
        request
            .speakers
            .iter()
            .map(|position| {
                let mut settings = ffi::IPLSourceSettings { flags: 3 };
                // SAFETY: Simulator is live, and the output handle is initially null.
                let source = Handle::new(&self.api, "source", self.api.iplSourceRelease, |handle| unsafe {
                    (self.api.iplSourceCreate)(simulator, &mut settings, handle)
                })?;
                let mut inputs = ffi::IPLSimulationInputs {
                    flags: 3,
                    directFlags: 1 | 2 | 8 | 16,
                    source: space(RoomListener { position: *position, forward: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0] }),
                    numOcclusionSamples: 1,
                    reverbScale: [1.0; 3],
                    hybridReverbTransitionTime: 1.0,
                    hybridReverbOverlapPercent: 0.25,
                    pathingOrder: 1,
                    numTransmissionRays: 1,
                    ..Default::default()
                };
                inputs.distanceAttenuationModel.minDistance = 1.0;
                inputs.directivity.dipolePower = 1.0;
                // SAFETY: Inputs contain validated positions and SDK enum values; sources remain owned through simulation.
                unsafe {
                    (self.api.iplSourceAdd)(source.raw, simulator);
                    (self.api.iplSourceSetInputs)(source.raw, 3, &mut inputs);
                }
                Ok(source)
            })
            .collect()
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        // SAFETY: Each handle is exclusively owned, and the dynamic library is still a live field.
        unsafe {
            for hrtf in self.hrtfs.values_mut() {
                (self.api.iplHRTFRelease)(hrtf);
            }
            if !self.embree.is_null() {
                (self.api.iplEmbreeDeviceRelease)(&mut self.embree);
            }
            (self.api.iplContextRelease)(&mut self.context);
        }
    }
}

struct Handle<'a, T> {
    raw: *mut T,
    release: unsafe extern "C" fn(*mut *mut T),
    _api: &'a ffi::Phonon,
}

impl<'a, T> Handle<'a, T> {
    fn new(
        api: &'a ffi::Phonon,
        operation: &'static str,
        release: unsafe extern "C" fn(*mut *mut T),
        call: impl FnOnce(*mut *mut T) -> i32,
    ) -> Result<Self, Error> {
        Ok(Self { raw: create(operation, release, call)?, release, _api: api })
    }
}

impl<T> Drop for Handle<'_, T> {
    fn drop(&mut self) {
        // SAFETY: The handle has one Rust owner, and its borrowed Phonon keeps release executable.
        unsafe { (self.release)(&mut self.raw) };
    }
}

fn create<T>(
    operation: &'static str,
    release: unsafe extern "C" fn(*mut *mut T),
    call: impl FnOnce(*mut *mut T) -> i32,
) -> Result<*mut T, Error> {
    let mut handle = ptr::null_mut();
    let status = call(&mut handle);
    if status != 0 || handle.is_null() {
        if !handle.is_null() {
            // SAFETY: Even a partially-created non-null handle belongs to this matching SDK release function.
            unsafe { release(&mut handle) };
        }
        return Err(Error::Native { operation, status });
    }
    Ok(handle)
}

struct Buffer<'a> {
    raw: ffi::IPLAudioBuffer,
    context: ffi::IPLContext,
    api: &'a ffi::Phonon,
}

impl<'a> Buffer<'a> {
    fn new(api: &'a ffi::Phonon, context: ffi::IPLContext, channels: i32) -> Result<Self, Error> {
        let mut raw = ffi::IPLAudioBuffer::default();
        // SAFETY: Fixed positive channel/frame counts and writable initialized output meet the SDK contract.
        let status = unsafe { (api.iplAudioBufferAllocate)(context, channels, FRAME as i32, &mut raw) };
        if status != 0 {
            return Err(Error::Native { operation: "audio buffer", status });
        }
        Ok(Self { raw, context, api })
    }
}

impl Drop for Buffer<'_> {
    fn drop(&mut self) {
        // SAFETY: This buffer was allocated once by this context and has not been freed or exposed to callers.
        unsafe { (self.api.iplAudioBufferFree)(self.context, &mut self.raw) };
    }
}

struct Effects<'a> {
    api: &'a ffi::Phonon,
    context: ffi::IPLContext,
    hrtf: ffi::IPLHRTF,
    reflection: Handle<'a, ffi::_IPLReflectionEffect_t>,
    decode: Handle<'a, ffi::_IPLAmbisonicsDecodeEffect_t>,
    direct: Handle<'a, ffi::_IPLDirectEffect_t>,
    ears: Ears<'a>,
    mono: Buffer<'a>,
    filtered: Buffer<'a>,
    ambisonic: Buffer<'a>,
    stereo: Buffer<'a>,
}

enum Ears<'a> {
    Headphones(Handle<'a, ffi::_IPLBinauralEffect_t>),
    Speakers(Handle<'a, ffi::_IPLPanningEffect_t>),
}

impl<'a> Effects<'a> {
    fn new(
        api: &'a ffi::Phonon,
        context: ffi::IPLContext,
        audio: &mut ffi::IPLAudioSettings,
        hrtf: ffi::IPLHRTF,
        request: &RoomRequest,
    ) -> Result<Self, Error> {
        let mut settings = ffi::IPLReflectionEffectSettings {
            type_: 0,
            irSize: (request.sample_rate * 2) as i32,
            numChannels: CHANNELS,
        };
        // SAFETY: All effect settings and returned handles are confined to this retained context and sample rate.
        let reflection = Handle::new(api, "reflection effect", api.iplReflectionEffectRelease, |handle| unsafe {
            (api.iplReflectionEffectCreate)(context, audio, &mut settings, handle)
        })?;
        let layout = ffi::IPLSpeakerLayout { type_: 1, numSpeakers: 2, speakers: ptr::null_mut() };
        let mut settings = ffi::IPLAmbisonicsDecodeEffectSettings { speakerLayout: layout, hrtf, maxOrder: ORDER };
        // SAFETY: HRTF is retained by Simulator, and the decoder accepts this stereo layout and ambisonic order.
        let decode = Handle::new(api, "ambisonics decode", api.iplAmbisonicsDecodeEffectRelease, |handle| unsafe {
            (api.iplAmbisonicsDecodeEffectCreate)(context, audio, &mut settings, handle)
        })?;
        let mut settings = ffi::IPLDirectEffectSettings { numChannels: 1 };
        // SAFETY: Direct processing uses the mono buffers allocated below.
        let direct = Handle::new(api, "direct effect", api.iplDirectEffectRelease, |handle| unsafe {
            (api.iplDirectEffectCreate)(context, audio, &mut settings, handle)
        })?;
        let ears = match request.output {
            RoomOutput::Headphones => {
                let mut settings = ffi::IPLBinauralEffectSettings { hrtf };
                // SAFETY: Binaural creation borrows the retained HRTF for this audio configuration.
                Ears::Headphones(Handle::new(api, "binaural effect", api.iplBinauralEffectRelease, |handle| unsafe {
                    (api.iplBinauralEffectCreate)(context, audio, &mut settings, handle)
                })?)
            }
            RoomOutput::Speakers => {
                let mut settings = ffi::IPLPanningEffectSettings { speakerLayout: layout };
                // SAFETY: Panning uses the SDK's built-in stereo speaker layout.
                Ears::Speakers(Handle::new(api, "panning effect", api.iplPanningEffectRelease, |handle| unsafe {
                    (api.iplPanningEffectCreate)(context, audio, &mut settings, handle)
                })?)
            }
        };
        Ok(Self {
            api,
            context,
            hrtf,
            reflection,
            decode,
            direct,
            ears,
            mono: Buffer::new(api, context, 1)?,
            filtered: Buffer::new(api, context, 1)?,
            ambisonic: Buffer::new(api, context, CHANNELS)?,
            stereo: Buffer::new(api, context, 2)?,
        })
    }

    fn speaker(
        &mut self,
        outputs: ffi::IPLSimulationOutputs,
        position: [f32; 3],
        request: &RoomRequest,
    ) -> SpeakerImpulse {
        let ir_size = request.sample_rate as usize * 2;
        let mut reflection = ffi::IPLReflectionEffectParams {
            type_: 0,
            numChannels: CHANNELS,
            irSize: ir_size as i32,
            ..outputs.reflections
        };
        let listener = space(request.listener);
        let mut decode = ffi::IPLAmbisonicsDecodeEffectParams {
            order: ORDER,
            hrtf: self.hrtf,
            orientation: listener,
            binaural: i32::from(request.output == RoomOutput::Headphones),
        };
        let wet = impulse(
            self.api,
            self.context,
            &mut self.mono.raw,
            &mut self.stereo.raw,
            ir_size + 2 * FRAME,
            |input, output| {
                // SAFETY: All buffers match effect channel counts; simulation outputs remain alive in the retained simulator.
                unsafe {
                    (self.api.iplReflectionEffectApply)(
                        self.reflection.raw,
                        &mut reflection,
                        input,
                        &mut self.ambisonic.raw,
                        ptr::null_mut(),
                    );
                    (self.api.iplAmbisonicsDecodeEffectApply)(
                        self.decode.raw,
                        &mut decode,
                        &mut self.ambisonic.raw,
                        output,
                    );
                }
            },
        );
        let mut direct = ffi::IPLDirectEffectParams { flags: 1 | 2 | 8 | 16, transmissionType: 1, ..outputs.direct };
        // SAFETY: Validated finite positions and an orthonormal listener frame satisfy direction calculation.
        let direction = unsafe {
            (self.api.iplCalculateRelativeDirection)(
                self.context,
                vector(position),
                listener.origin,
                listener.ahead,
                listener.up,
            )
        };
        let dry = impulse(self.api, self.context, &mut self.mono.raw, &mut self.stereo.raw, FRAME, |input, output| {
            // SAFETY: Both direct effect buffers are mono; ears renders the result into the stereo buffer.
            unsafe {
                (self.api.iplDirectEffectApply)(self.direct.raw, &mut direct, input, &mut self.filtered.raw);
            }
            self.ears.apply(self.api, self.hrtf, direction, &mut self.filtered.raw, output);
        });
        let onset =
            impulse(self.api, self.context, &mut self.mono.raw, &mut self.stereo.raw, FRAME, |input, output| {
                self.ears.apply(self.api, self.hrtf, direction, input, output)
            })
            .iter()
            .filter_map(|channel| channel.iter().position(|value| value.abs() > 1e-6))
            .min()
            .unwrap_or(0);
        let distance =
            position.iter().zip(request.listener.position).map(|(a, b)| f64::from(*a - b).powi(2)).sum::<f64>().sqrt();
        SpeakerImpulse { wet, dry, onset, delay: (distance / 340.0 * f64::from(request.sample_rate)).round() as usize }
    }
}

impl Ears<'_> {
    fn apply(
        &self,
        api: &ffi::Phonon,
        hrtf: ffi::IPLHRTF,
        direction: ffi::IPLVector3,
        input: &mut ffi::IPLAudioBuffer,
        output: &mut ffi::IPLAudioBuffer,
    ) {
        // SAFETY: Input is mono, output is stereo, and each effect was created for the retained HRTF/audio rate.
        unsafe {
            match self {
                Self::Headphones(effect) => {
                    let mut params = ffi::IPLBinauralEffectParams {
                        direction,
                        interpolation: 1,
                        spatialBlend: 1.0,
                        hrtf,
                        peakDelays: ptr::null_mut(),
                    };
                    (api.iplBinauralEffectApply)(effect.raw, &mut params, input, output);
                }
                Self::Speakers(effect) => {
                    let mut params = ffi::IPLPanningEffectParams { direction };
                    (api.iplPanningEffectApply)(effect.raw, &mut params, input, output);
                }
            }
        }
    }
}

fn impulse(
    api: &ffi::Phonon,
    context: ffi::IPLContext,
    input: &mut ffi::IPLAudioBuffer,
    output: &mut ffi::IPLAudioBuffer,
    length: usize,
    mut chain: impl FnMut(&mut ffi::IPLAudioBuffer, &mut ffi::IPLAudioBuffer),
) -> [Vec<f32>; 2] {
    let mut result = [vec![0.0; length], vec![0.0; length]];
    let mut samples = [0.0_f32; FRAME];
    let mut interleaved = [0.0_f32; FRAME * 2];
    for frame in 0..2 + length.div_ceil(FRAME) {
        samples.fill(0.0);
        if frame == 2 {
            samples[0] = 1.0;
        }
        // SAFETY: The interleaved input contains exactly FRAME samples for the SDK's mono buffer.
        unsafe {
            (api.iplAudioBufferDeinterleave)(context, samples.as_mut_ptr(), input);
        }
        chain(input, output);
        // SAFETY: The output vector has FRAME samples for each of the SDK stereo buffer's two channels.
        unsafe {
            (api.iplAudioBufferInterleave)(context, output, interleaved.as_mut_ptr());
        }
        if frame < 2 {
            continue;
        }
        let start = (frame - 2) * FRAME;
        for index in 0..FRAME.min(length - start) {
            result[0][start + index] = interleaved[index * 2];
            result[1][start + index] = interleaved[index * 2 + 1];
        }
    }
    result
}

fn vector([x, y, z]: [f32; 3]) -> ffi::IPLVector3 {
    ffi::IPLVector3 { x, y, z }
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalized(value: [f32; 3]) -> [f32; 3] {
    let length = value.iter().map(|value| value * value).sum::<f32>().sqrt();
    value.map(|value| value / length)
}
fn space(listener: RoomListener) -> ffi::IPLCoordinateSpace3 {
    let ahead = normalized(listener.forward);
    let right = normalized(cross(ahead, listener.up));
    ffi::IPLCoordinateSpace3 {
        right: vector(right),
        up: vector(cross(right, ahead)),
        ahead: vector(ahead),
        origin: vector(listener.position),
    }
}

fn validate(request: &RoomRequest) -> Result<(), Error> {
    if !(8000..=192000).contains(&request.sample_rate) {
        return Err(Error::Invalid("sample rate"));
    }
    let finite = |vector: [f32; 3]| vector.iter().all(|value| value.is_finite() && value.abs() <= 1e6);
    if !request.speakers.iter().copied().all(finite)
        || ![request.listener.position, request.listener.forward, request.listener.up].into_iter().all(finite)
        || cross(request.listener.forward, request.listener.up).iter().map(|v| v * v).sum::<f32>() <= 1e-12
    {
        return Err(Error::Invalid("listener or speaker position"));
    }
    let world = &request.world;
    if world.meshes.is_empty() || world.materials.is_empty() || world.materials.len() > 65536 {
        return Err(Error::Invalid("room geometry or materials"));
    }
    for material in &world.materials {
        if !material
            .absorption
            .iter()
            .chain(&material.transmission)
            .chain(std::iter::once(&material.scattering))
            .all(|value| (0.0..=1.0).contains(value))
        {
            return Err(Error::Invalid("acoustic material"));
        }
    }
    let mut count = 0usize;
    for mesh in &world.meshes {
        count = count.saturating_add(mesh.triangles.len());
        if count > 2_000_000
            || mesh.positions.is_empty()
            || mesh.positions.len() > i32::MAX as usize
            || mesh.triangles.len() != mesh.triangle_materials.len()
            || !mesh.positions.iter().copied().all(finite)
        {
            return Err(Error::Invalid("mesh array dimensions"));
        }
        if mesh.triangles.iter().flatten().any(|index| *index as usize >= mesh.positions.len())
            || mesh.triangle_materials.iter().any(|index| usize::from(*index) >= world.materials.len())
        {
            return Err(Error::Invalid("mesh index"));
        }
    }
    Ok(())
}
