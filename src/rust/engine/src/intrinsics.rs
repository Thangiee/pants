use crate::context::Context;
use crate::core::{throw, Value};
use crate::externs;
use crate::nodes::MultiPlatformExecuteProcess;
use crate::nodes::{lift_digest, DownloadedFile, NodeFuture, Snapshot};
use crate::tasks::Intrinsic;
use crate::types::Types;

use boxfuture::Boxable;
use bytes;
use futures::future::{self as future03, TryFutureExt};
use futures01::{future, Future};
use hashing;
use indexmap::IndexMap;

use std::path::PathBuf;

type IntrinsicFn = Box<dyn Fn(Context, Vec<Value>) -> NodeFuture<Value> + Send + Sync + 'static>;

pub struct Intrinsics {
  intrinsics: IndexMap<Intrinsic, IntrinsicFn>,
}

impl Intrinsics {
  pub fn new(types: &Types) -> Intrinsics {
    let mut intrinsics: IndexMap<Intrinsic, IntrinsicFn> = IndexMap::new();
    intrinsics.insert(
      Intrinsic {
        product: types.directory_digest,
        inputs: vec![types.input_files_content],
      },
      Box::new(input_files_content_to_digest),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.snapshot,
        inputs: vec![types.path_globs],
      },
      Box::new(path_globs_to_snapshot),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.snapshot,
        inputs: vec![types.url_to_fetch],
      },
      Box::new(url_to_fetch_to_snapshot),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.snapshot,
        inputs: vec![types.directory_digest],
      },
      Box::new(digest_to_snapshot),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.files_content,
        inputs: vec![types.directory_digest],
      },
      Box::new(directory_digest_to_files_content),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.directory_digest,
        inputs: vec![types.merge_digests],
      },
      Box::new(merge_digests_request_to_digest),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.directory_digest,
        inputs: vec![types.remove_prefix],
      },
      Box::new(remove_prefix_request_to_digest),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.directory_digest,
        inputs: vec![types.add_prefix],
      },
      Box::new(add_prefix_request_to_digest),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.process_result,
        inputs: vec![types.multi_platform_process_request, types.platform],
      },
      Box::new(multi_platform_process_request_to_process_result),
    );
    intrinsics.insert(
      Intrinsic {
        product: types.snapshot,
        inputs: vec![types.snapshot_subset],
      },
      Box::new(snapshot_subset_to_snapshot),
    );
    Intrinsics { intrinsics }
  }

  pub fn keys(&self) -> impl Iterator<Item = &Intrinsic> {
    self.intrinsics.keys()
  }

  pub fn run(&self, intrinsic: Intrinsic, context: Context, args: Vec<Value>) -> NodeFuture<Value> {
    let function = self
      .intrinsics
      .get(&intrinsic)
      .unwrap_or_else(|| panic!("Unrecognized intrinsic: {:?}", intrinsic));
    function(context, args)
  }
}

fn multi_platform_process_request_to_process_result(
  context: Context,
  args: Vec<Value>,
) -> NodeFuture<Value> {
  let process_val = &args[0];
  // TODO: The platform will be used in a followup.
  let _platform_val = &args[1];
  let core = context.core.clone();
  future::result(
    MultiPlatformExecuteProcess::lift(process_val).map_err(|str| {
      throw(&format!(
        "Error lifting MultiPlatformExecuteProcess: {}",
        str
      ))
    }),
  )
  .and_then(move |process_request| context.get(process_request))
  .map(move |result| {
    let platform_name: String = result.0.platform.into();
    externs::unsafe_call(
      &core.types.construct_process_result,
      &[
        externs::store_bytes(&result.0.stdout),
        externs::store_bytes(&result.0.stderr),
        externs::store_i64(result.0.exit_code.into()),
        Snapshot::store_directory(&core, &result.0.output_directory),
        externs::unsafe_call(
          &core.types.construct_platform,
          &[externs::store_utf8(&platform_name)],
        ),
      ],
    )
  })
  .to_boxed()
}

fn directory_digest_to_files_content(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  future::result(lift_digest(&args[0]).map_err(|str| throw(&str)))
    .and_then(move |digest| {
      context
        .core
        .store()
        .contents_for_directory(digest)
        .map_err(|str| throw(&str))
        .map(move |files_content| Snapshot::store_files_content(&context, &files_content))
    })
    .to_boxed()
}

fn remove_prefix_request_to_digest(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  let core = context.core;

  Box::pin(async move {
    let input_digest = lift_digest(&externs::project_ignoring_type(&args[0], "digest"))?;
    let prefix = externs::project_str(&args[0], "prefix");
    let digest =
      store::Snapshot::strip_prefix(core.store(), input_digest, PathBuf::from(prefix)).await?;
    let res: Result<_, String> = Ok(Snapshot::store_directory(&core, &digest));
    res
  })
  .compat()
  .map_err(|e: String| throw(&e))
  .to_boxed()
}

fn add_prefix_request_to_digest(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  let core = context.core;
  Box::pin(async move {
    let input_digest = lift_digest(&externs::project_ignoring_type(&args[0], "digest"))?;
    let prefix = externs::project_str(&args[0], "prefix");
    let digest =
      store::Snapshot::add_prefix(core.store(), input_digest, PathBuf::from(prefix)).await?;
    let res: Result<_, String> = Ok(Snapshot::store_directory(&core, &digest));
    res
  })
  .compat()
  .map_err(|e: String| throw(&e))
  .to_boxed()
}

fn digest_to_snapshot(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  let core = context.core.clone();
  let store = context.core.store();
  Box::pin(async move {
    let digest = lift_digest(&args[0])?;
    let snapshot = store::Snapshot::from_digest(store, digest).await?;
    let res: Result<_, String> = Ok(Snapshot::store_snapshot(&core, &snapshot));
    res
  })
  .compat()
  .map_err(|e: String| throw(&e))
  .to_boxed()
}

fn merge_digests_request_to_digest(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  let core = context.core;
  let digests: Result<Vec<hashing::Digest>, String> = externs::project_multi(&args[0], "digests")
    .into_iter()
    .map(|val| lift_digest(&val))
    .collect();
  Box::pin(async move {
    let digest = store::Snapshot::merge_directories(core.store(), digests?).await?;
    let res: Result<_, String> = Ok(Snapshot::store_directory(&core, &digest));
    res
  })
  .compat()
  .map_err(|err: String| throw(&err))
  .to_boxed()
}

fn url_to_fetch_to_snapshot(context: Context, mut args: Vec<Value>) -> NodeFuture<Value> {
  let core = context.core.clone();
  context
    .get(DownloadedFile(externs::key_for(args.pop().unwrap())))
    .map(move |snapshot| Snapshot::store_snapshot(&core, &snapshot))
    .to_boxed()
}

fn path_globs_to_snapshot(context: Context, mut args: Vec<Value>) -> NodeFuture<Value> {
  let core = context.core.clone();
  context
    .get(Snapshot(externs::key_for(args.pop().unwrap())))
    .map(move |snapshot| Snapshot::store_snapshot(&core, &snapshot))
    .to_boxed()
}

fn input_files_content_to_digest(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  let file_values = externs::project_multi(&args[0], "dependencies");
  let digests: Vec<_> = file_values
    .iter()
    .map(|file| {
      let filename = externs::project_str(&file, "path");
      let path: PathBuf = filename.into();
      let bytes = bytes::Bytes::from(externs::project_bytes(&file, "content"));
      let is_executable = externs::project_bool(&file, "is_executable");

      let store = context.core.store();
      async move {
        let digest = store.store_file_bytes(bytes, true).await?;
        let snapshot = store
          .snapshot_of_one_file(path, digest, is_executable)
          .await?;
        let res: Result<_, String> = Ok(snapshot.digest);
        res
      }
    })
    .collect();

  Box::pin(async move {
    let digests = future03::try_join_all(digests).await?;
    let digest = store::Snapshot::merge_directories(context.core.store(), digests).await?;
    let res: Result<_, String> = Ok(Snapshot::store_directory(&context.core, &digest));
    res
  })
  .compat()
  .map_err(|err: String| throw(&err))
  .to_boxed()
}

fn snapshot_subset_to_snapshot(context: Context, args: Vec<Value>) -> NodeFuture<Value> {
  let globs = externs::project_ignoring_type(&args[0], "globs");
  let store = context.core.store();

  Box::pin(async move {
    let path_globs = Snapshot::lift_path_globs(&globs)?;
    let original_digest = lift_digest(&externs::project_ignoring_type(&args[0], "digest"))?;

    let snapshot = store::Snapshot::get_snapshot_subset(store, original_digest, path_globs).await?;

    Ok(Snapshot::store_snapshot(&context.core, &snapshot))
  })
  .compat()
  .map_err(|err: String| throw(&err))
  .to_boxed()
}
