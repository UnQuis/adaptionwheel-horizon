// AdaptionWheel — metal wheel surface, translated from the original GLSL
// `shaders/core/wheel_metal.{vsh,fsh}` to Horizon's restricted pipeline shader.
//
// The original shader needed custom uniforms (eye, wheel centre, sun direction,
// eight nearby emitters) that Horizon's pipeline stage does not expose. What
// transfers is the look: a tarnished steel base, a reflected sky/ground band,
// studio grid lines that slide with the surface, and a Fresnel rim. All of it
// is derived from the quad's own UV/position and the game's time.

fn wheel_hash(p: vec3<f32>) -> f32 {
    let q = fract(p * 0.1031 + vec3<f32>(0.1, 0.2, 0.3));
    let r = q * 17.0;
    return fract(r.x * r.y * r.z * (r.x + r.y + r.z));
}

fn wheel_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    let n000 = wheel_hash(i);
    let n100 = wheel_hash(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = wheel_hash(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = wheel_hash(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = wheel_hash(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = wheel_hash(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = wheel_hash(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = wheel_hash(i + vec3<f32>(1.0, 1.0, 1.0));
    return mix(
        mix(mix(n000, n100, s.x), mix(n010, n110, s.x), s.y),
        mix(mix(n001, n101, s.x), mix(n011, n111, s.x), s.y),
        s.z,
    );
}

fn wheel_fbm(p: vec3<f32>) -> f32 {
    let a = wheel_noise(p) * 0.5;
    let b = wheel_noise(p * 2.03) * 0.25;
    let c = wheel_noise(p * 4.12) * 0.125;
    let d = wheel_noise(p * 8.37) * 0.0625;
    return a + b + c + d;
}

fn wheel_line(v: f32, n: f32, w: f32) -> f32 {
    return smoothstep(0.5 - w, 0.5, abs(fract(v * n) - 0.5));
}

fn pipeline_fragment(input: PipelineInput) -> vec4<f32> {
    let uv = input.uv;
    let p = input.position;

    // Four-octave corrosion/wear, keyed to the wheel's own coordinates so the
    // pattern stays with the metal as it turns.
    let wear = wheel_fbm(p * 9.0 + vec3<f32>(uv.x * 2.0, uv.y * 2.0, frame.time * 0.02));
    let tarnish = smoothstep(0.35, 0.72, wear);

    // Reflected sky/ground band: a bright horizon line is what makes the eye
    // read polished metal, exactly as in the original `environment()`.
    let up = uv.y * 2.0 - 1.0;
    let sky = mix(vec3<f32>(0.34, 0.40, 0.48), vec3<f32>(0.78, 0.86, 1.00), smoothstep(-0.1, 0.9, up));
    let ground = vec3<f32>(0.16, 0.13, 0.11);
    let env = mix(ground, sky, smoothstep(-0.06, 0.06, up));

    // The reflected studio grid, sliding with the surface, plus a soft strip.
    let az = atan2(p.x, p.z) * 0.15915494;
    let grid = wheel_line(az, 8.0, 0.012) * 0.75 + wheel_line(up, 6.0, 0.010) * 0.45;
    let strip = pow(max(0.0, cos(az * 6.2831853 + 1.2)), 24.0) * smoothstep(0.0, 0.3, up);
    let reflection = env + vec3<f32>(0.95, 0.97, 1.0) * (grid * 0.42 + strip * 0.30) * (1.0 - 0.7 * tarnish);

    // Fresnel floor: metal reflects strongly even face-on (liquid-chrome idea).
    let fresnel = 0.6 + 0.4 * pow(1.0 - abs(up), 3.0);
    let steel = input.color.rgb * mix(vec3<f32>(0.86, 0.88, 0.92), vec3<f32>(0.42, 0.36, 0.30), tarnish);
    let color = mix(steel, reflection, 0.55 * fresnel) + vec3<f32>(0.08) * (1.0 - tarnish) * strip;

    return vec4<f32>(color, input.color.a);
}
