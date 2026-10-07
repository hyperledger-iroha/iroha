//! One Android install export feeding the same existing C/JNI runtime registry.

use super::*;
use ::jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObject},
    sys::jlong,
};

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletInstalledRuntimeNativeV1_installRuntime(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    platform: JObject<'_>,
    app_manifest: JByteArray<'_>,
    envelope: JByteArray<'_>,
    wallet_runtime: JByteArray<'_>,
    verifier_pack: JByteArray<'_>,
    producer_inventory: JByteArray<'_>,
    signed_genesis: JByteArray<'_>,
    originals_root: JByteArray<'_>,
) -> jlong {
    let result = run(|| {
        let arrays = [
            &app_manifest,
            &envelope,
            &wallet_runtime,
            &verifier_pack,
            &producer_inventory,
            &signed_genesis,
            &originals_root,
        ];
        // Check every original extent before copying any Java array or invoking platform.
        for (array, bound) in arrays.into_iter().zip(RUNTIME_BOUNDS) {
            let length = env
                .get_array_length(array)
                .map_err(|_| Failure::code(INVALID))?;
            if length < 0 || length as usize > bound {
                return Err(Failure::code(INVALID));
            }
        }
        let originals: Vec<Vec<u8>> = arrays
            .into_iter()
            .map(|array| {
                env.convert_byte_array(array)
                    .map_err(|_| Failure::code(INVALID))
            })
            .collect::<Result<_>>()?;
        let prepared = PreparedInstallation::load(RuntimeOriginals {
            app_manifest: &originals[0],
            envelope: &originals[1],
            wallet_runtime: &originals[2],
            verifier_pack: &originals[3],
            producer_inventory: &originals[4],
            signed_genesis: &originals[5],
            originals_root: &originals[6],
        })?;
        let platform = AndroidPlatform::new(&mut env, &platform)?;
        let root = platform
            .custody_root()
            .map_err(|error| Failure::unavailable(UNAVAILABLE, error))?;
        prepared.register(platform, root, true)
    });
    match result {
        Ok(handle) => handle as jlong,
        Err(error) => error.status as jlong,
    }
}
