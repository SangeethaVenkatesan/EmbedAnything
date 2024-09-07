pub mod config;

use embed_anything::{
    self, config::TextEmbedConfig, emb_audio, embeddings::embed::Embeder,
    file_processor::audio::audio_processor, text_loader::FileLoadingError,
};
use pyo3::{exceptions::PyValueError, exceptions::PyFileNotFoundError, prelude::*};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::runtime::Builder;

#[pyclass]
pub struct EmbedData {
    pub inner: embed_anything::embeddings::embed::EmbedData,
}

#[pymethods]
impl EmbedData {
    #[getter(embedding)]
    fn embedding(&self) -> Vec<f32> {
        self.inner.embedding.clone()
    }

    #[getter(text)]
    fn text(&self) -> Option<String> {
        self.inner.text.clone()
    }

    #[getter(metadata)]
    fn metadata(&self) -> Option<HashMap<String, String>> {
        self.inner.metadata.clone()
    }

    fn __str__(&self) -> String {
        format!(
            "EmbedData(embedding: {:?}, text: {:?}, metadata: {:?})",
            self.inner.embedding,
            self.inner.text,
            self.inner.metadata.clone()
        )
    }

    fn __repr__(&self) -> String {
        "<class 'EmbedData'>".to_string()
    }
}

#[pyclass(eq, eq_int)]
#[derive(PartialEq)]
pub enum WhichModel {
    OpenAI,
    Cohere,
    Bert,
    Clip,
    Jina,
}

impl From<&str> for WhichModel {
    fn from(s: &str) -> Self {
        match s {
            "openai" | "OpenAI" => WhichModel::OpenAI,
            "cohere" | "Cohere" => WhichModel::Cohere,
            "bert" | "Bert" => WhichModel::Bert,
            "clip" | "Clip" => WhichModel::Clip,
            "jina" | "Jina" => WhichModel::Jina,
            _ => panic!("Invalid model"),
        }
    }
}

impl From<String> for WhichModel {
    fn from(s: String) -> Self {
        match s.as_str() {
            "openai" | "OpenAI" => WhichModel::OpenAI,
            "cohere" | "Cohere" => WhichModel::Cohere,
            "bert" | "Bert" => WhichModel::Bert,
            "clip" | "Clip" => WhichModel::Clip,
            "jina" | "Jina" => WhichModel::Jina,
            _ => panic!("Invalid model"),
        }
    }
}

#[pyclass]
pub struct EmbeddingModel {
    pub inner: Arc<Embeder>,
}

#[pymethods]
impl EmbeddingModel {
    #[staticmethod]
    #[pyo3(signature = (model, model_id, revision=None))]
    fn from_pretrained_hf(
        model: &WhichModel,
        model_id: Option<&str>,
        revision: Option<&str>,
    ) -> PyResult<Self> {
        // let model = WhichModel::from(model);
        match model {
            WhichModel::Bert => {
                let model_id = model_id.unwrap_or("sentence-transformers/all-MiniLM-L12-v2");
                let model = Embeder::Bert(
                    embed_anything::embeddings::local::bert::BertEmbeder::new(
                        model_id.to_string(),
                        revision.map(|s| s.to_string()),
                    )
                    .unwrap(),
                );
                Ok(EmbeddingModel {
                    inner: Arc::new(model),
                })
            }
            WhichModel::Clip => {
                let model_id = model_id.unwrap_or("openai/clip-vit-base-patch32");
                let model = Embeder::Clip(
                    embed_anything::embeddings::local::clip::ClipEmbeder::new(
                        model_id.to_string(),
                        revision.map(|s| s.to_string()),
                    )
                    .map_err(|e| PyValueError::new_err(e.to_string()))?,
                );
                Ok(EmbeddingModel {
                    inner: Arc::new(model),
                })
            }
            WhichModel::Jina => {
                let model_id = model_id.unwrap_or("jinaai/jina-embeddings-v2-small-en");
                let model = Embeder::Jina(
                    embed_anything::embeddings::local::jina::JinaEmbeder::new(
                        model_id.to_string(),
                        revision.map(|s| s.to_string()),
                    )
                    .unwrap(),
                );
                Ok(EmbeddingModel {
                    inner: Arc::new(model),
                })
            }
            _ => panic!("Invalid model"),
        }
    }

    #[staticmethod]
    #[pyo3(signature = (model, model_id,  api_key=None))]
    fn from_pretrained_cloud(
        model: &WhichModel,
        model_id: Option<&str>,
        api_key: Option<String>,
    ) -> PyResult<Self> {
        match model {
            WhichModel::OpenAI => {
                let model_id = model_id.unwrap_or("text-embedding-3-small");
                let model = Embeder::OpenAI(
                    embed_anything::embeddings::cloud::openai::OpenAIEmbeder::new(
                        model_id.to_string(),
                        api_key,
                    ),
                );
                Ok(EmbeddingModel {
                    inner: Arc::new(model),
                })
            }
            WhichModel::Cohere => {
                let model_id = model_id.unwrap_or("embed-english-v3.0");
                let model = Embeder::Cohere(
                    embed_anything::embeddings::cloud::cohere::CohereEmbeder::new(
                        model_id.to_string(),
                        api_key,
                    ),
                );
                Ok(EmbeddingModel {
                    inner: Arc::new(model),
                })
            }
            _ => panic!("Invalid model"),
        }
    }
}

#[pyclass]
pub struct AudioDecoderModel {
    pub inner: audio_processor::AudioDecoderModel,
}

#[pymethods]
impl AudioDecoderModel {
    #[staticmethod]
    #[pyo3(signature = (model_id, revision=None, model_type=None, quantized=None))]
    fn from_pretrained_hf(
        model_id: Option<&str>,
        revision: Option<&str>,
        model_type: Option<&str>,
        quantized: Option<bool>,
    ) -> PyResult<Self> {
        let model_id = model_id.unwrap_or("openai/whisper-tiny.en");
        let model_type = model_type.unwrap_or("tiny-en");
        let revision = revision.unwrap_or("main");
        let model = audio_processor::AudioDecoderModel::from_pretrained(
            Some(model_id),
            Some(revision),
            model_type,
            quantized.unwrap_or(false),
        )
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(AudioDecoderModel { inner: model })
    }
}

#[pyfunction]
#[pyo3(signature = (query, embeder, config=None))]
pub fn embed_query(
    query: Vec<String>,
    embeder: &EmbeddingModel,
    config: Option<&config::TextEmbedConfig>,
) -> PyResult<Vec<EmbedData>> {
    let config = config.map(|c| &c.inner);
    let embedding_model = &embeder.inner;

    Ok(embed_anything::embed_query(
        query,
        embedding_model,
        Some(config.unwrap_or(&TextEmbedConfig::default())),
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))?
    .into_iter()
    .map(|data| EmbedData { inner: data })
    .collect())
}

#[pyfunction]
#[pyo3(signature = (file_name, embeder, config=None, adapter=None))]
pub fn embed_file(
    file_name: &str,
    embeder: &EmbeddingModel,
    config: Option<&config::TextEmbedConfig>,
    adapter: Option<PyObject>,
) -> PyResult<Option<Vec<EmbedData>>> {
    let config = config.map(|c| &c.inner);
    let embedding_model = &embeder.inner;
    if !Path::new(file_name).exists() {
        // check if the file exists other wise return a "File not found" error with PyValueError
        return Err(PyFileNotFoundError::new_err(format!(
            "File not found: {:?}",
            file_name
        )));
    };
    let adapter = match adapter {
        Some(adapter) => {
            let callback = move |data: Vec<embed_anything::embeddings::embed::EmbedData>| {
                Python::with_gil(|py| {
                    let upsert_fn = adapter.getattr(py, "upsert").unwrap();
                    let converted_data = data
                        .into_iter()
                        .map(|data| EmbedData { inner: data })
                        .collect::<Vec<EmbedData>>();
                    upsert_fn
                        .call1(py, (converted_data,))
                        .map_err(|e| PyValueError::new_err(e.to_string()))
                        .unwrap();
                });
            };
            Some(callback)
        }
        None => None,
    };

    let data = embed_anything::embed_file(file_name, &embedding_model, config, adapter)
        .map_err(|e| {
            if let Some(file_loading_error) = e.downcast_ref::<FileLoadingError>() {
                match file_loading_error {
                    FileLoadingError::FileNotFound(file) => {
                        PyFileNotFoundError::new_err(file.clone())
                    }
                    FileLoadingError::UnsupportedFileType(file) => {
                        PyValueError::new_err(file.clone())
                    }
                }
            } else {
                PyValueError::new_err(e.to_string())
            }
        })?
        .map(|data| {
            data.into_iter()
                .map(|data| EmbedData { inner: data })
                .collect::<Vec<_>>()
        });
    Ok(data)
}

#[pyfunction]
#[pyo3(signature = (audio_file, audio_decoder, embeder, text_embed_config=None))]
pub fn embed_audio_file(
    audio_file: String,
    audio_decoder: &mut AudioDecoderModel,
    embeder: &EmbeddingModel,
    text_embed_config: Option<&config::TextEmbedConfig>,
) -> PyResult<Option<Vec<EmbedData>>> {
    let config = text_embed_config.map(|c| &c.inner);
    let embedding_model = &embeder.inner;
    let audio_decoder = &mut audio_decoder.inner;

    let data = emb_audio(audio_file, audio_decoder, embedding_model, config)
        .map_err(|e| PyValueError::new_err(e.to_string()))?
        .map(|data| {
            data.into_iter()
                .map(|data| EmbedData { inner: data })
                .collect::<Vec<_>>()
        });
    Ok(data)
}

#[pyfunction]
#[pyo3(signature = (directory, embeder, extensions=None, config=None, adapter = None))]
pub fn embed_directory(
    directory: PathBuf,
    embeder: &EmbeddingModel,
    extensions: Option<Vec<String>>,
    config: Option<&config::TextEmbedConfig>,
    adapter: Option<PyObject>,
) -> PyResult<Option<Vec<EmbedData>>> {
    let config = config.map(|c| &c.inner);
    let embedding_model = &embeder.inner;

    let rt = Builder::new_multi_thread().enable_all().build().unwrap();
    println!("Runtime created");
    let adapter = match adapter {
        Some(adapter) => {
            let callback = move |data: Vec<embed_anything::embeddings::embed::EmbedData>| {
                Python::with_gil(|py| {
                    let upsert_fn = adapter.getattr(py, "upsert").unwrap();
                    let converted_data = data
                        .into_iter()
                        .map(|data| EmbedData { inner: data })
                        .collect::<Vec<EmbedData>>();
                    upsert_fn
                        .call1(py, (converted_data,))
                        .map_err(|e| PyValueError::new_err(e.to_string()))
                        .unwrap();
                })
            };
            Some(callback)
        }
        None => None,
    };

    let data = rt.block_on(async {
        embed_anything::embed_directory_stream(
            directory,
            embedding_model,
            extensions,
            config,
            adapter,
        )
        .await
        .map_err(|e| PyValueError::new_err(e.to_string()))
        .unwrap()
        .map(|data| {
            data.into_iter()
                .map(|data| EmbedData { inner: data })
                .collect::<Vec<_>>()
        })
    });
    Ok(data)
}

#[pyfunction]
#[pyo3(signature = (directory, embeder, config=None, adapter = None))]
pub fn embed_image_directory(
    directory: PathBuf,
    embeder: &EmbeddingModel,
    config: Option<&config::ImageEmbedConfig>,
    adapter: Option<PyObject>,
) -> PyResult<Option<Vec<EmbedData>>> {
    let embedding_model = &embeder.inner;
    let config = config.map(|c| &c.inner);
    let rt = Builder::new_multi_thread().enable_all().build().unwrap();
    println!("Runtime created");

    let adapter = match adapter {
        Some(adapter) => {
            let callback = move |data: Vec<embed_anything::embeddings::embed::EmbedData>| {
                Python::with_gil(|py| {
                    let upsert_fn = adapter.getattr(py, "upsert").unwrap();
                    let converted_data = data
                        .into_iter()
                        .map(|data| EmbedData { inner: data })
                        .collect::<Vec<EmbedData>>();
                    upsert_fn
                        .call1(py, (converted_data,))
                        .map_err(|e| PyValueError::new_err(e.to_string()))
                        .unwrap();
                });
            };
            Some(callback)
        }
        None => None,
    };

    let data = rt.block_on(async {
        embed_anything::embed_image_directory(directory, embedding_model, config, adapter)
            .await
            .map_err(|e| PyValueError::new_err(e.to_string()))
            .unwrap()
            .map(|data| {
                data.into_iter()
                    .map(|data| EmbedData { inner: data })
                    .collect::<Vec<_>>()
            })
    });
    Ok(data)
}
#[pyfunction]
#[pyo3(signature = (url, embeder, config=None, adapter = None))]
pub fn embed_webpage(
    url: String,
    embeder: &EmbeddingModel,
    config: Option<&config::TextEmbedConfig>,
    adapter: Option<PyObject>,
) -> PyResult<Option<Vec<EmbedData>>> {
    let embedding_model = &embeder.inner;
    let config = config.map(|c| &c.inner);

    let adapter = match adapter {
        Some(adapter) => {
            let callback = move |data: Vec<embed_anything::embeddings::embed::EmbedData>| {
                Python::with_gil(|py| {
                    let upsert_fn = adapter.getattr(py, "upsert").unwrap();
                    let converted_data = data
                        .into_iter()
                        .map(|data| EmbedData { inner: data })
                        .collect::<Vec<EmbedData>>();
                    upsert_fn
                        .call1(py, (converted_data,))
                        .map_err(|e| PyValueError::new_err(e.to_string()))
                        .unwrap();
                });
            };
            Some(callback)
        }
        None => None,
    };

    let data = embed_anything::embed_webpage(url, embedding_model, config, adapter)
        .map_err(|e| PyValueError::new_err(e.to_string()))?
        .map(|data| {
            data.into_iter()
                .map(|data| EmbedData { inner: data })
                .collect::<Vec<_>>()
        });
    Ok(data)
}

#[pymodule]
fn _embed_anything(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(embed_file, m)?)?;
    m.add_function(wrap_pyfunction!(embed_directory, m)?)?;
    m.add_function(wrap_pyfunction!(embed_image_directory, m)?)?;
    m.add_function(wrap_pyfunction!(embed_query, m)?)?;
    m.add_function(wrap_pyfunction!(embed_webpage, m)?)?;
    m.add_function(wrap_pyfunction!(embed_audio_file, m)?)?;
    m.add_class::<EmbeddingModel>()?;
    m.add_class::<AudioDecoderModel>()?;
    m.add_class::<WhichModel>()?;
    m.add_class::<EmbedData>()?;
    m.add_class::<config::TextEmbedConfig>()?;
    Ok(())
}