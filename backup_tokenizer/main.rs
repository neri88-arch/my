use anyhow::{anyhow, Result};
use arrow::array::AsArray;
use arrow::datatypes::DataType;
use clap::{Parser, Subcommand};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rayon::prelude::*;
use std::fs::File;
use std::sync::Arc;
use std::io::{BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tokenizers::models::bpe::{BpeTrainerBuilder, BPE};
use tokenizers::models::TrainerWrapper;
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::pre_tokenizers::sequence::Sequence;
use tokenizers::pre_tokenizers::split::{Split, SplitPattern};
use tokenizers::processors::PostProcessorWrapper;
use tokenizers::tokenizer::{AddedToken, SplitDelimiterBehavior, Tokenizer};
use unicode_normalization::UnicodeNormalization;

#[derive(Parser)]
#[command(name = "parquet_tokenizer_rs")]
#[command(about = "Addestra tokenizer BPE da Parquet e produce token binari")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Addestra il tokenizer da file Parquet (solo 000-003)
    Train {
        /// File Parquet
        #[arg(long, required = true)]
        input: Vec<PathBuf>,

        /// Colonna testo
        #[arg(long, default_value = "text")]
        text_column: String,

        /// Output tokenizer.json
        #[arg(long, default_value = "tokenizer.json")]
        output: PathBuf,

        /// Dimensione vocabolario
        #[arg(long, default_value_t = 65536)]
        vocab_size: usize,

        /// Frequenza minima
        #[arg(long, default_value_t = 2)]
        min_frequency: u64,

        /// Pulisce URL ed email durante il training
        #[arg(long)]
        clean: bool,

        /// Numero minimo di caratteri per documento
        #[arg(long, default_value_t = 32)]
        min_chars: usize,
    },

    /// Tokenizza Parquet in un file binario
    Encode {
        /// File Parquet
        #[arg(long, required = true)]
        input: Vec<PathBuf>,

        /// Tokenizer da usare
        #[arg(long)]
        tokenizer: PathBuf,

        /// Output binario
        #[arg(long)]
        output: PathBuf,

        /// Colonna testo
        #[arg(long, default_value = "text")]
        text_column: String,

        /// Lunghezza sequenze, 0 = flusso piatto (nessun packing)
        #[arg(long, default_value_t = 4096)]
        seq_len: usize,

        /// Pulisce il testo prima di tokenizzare
        #[arg(long)]
        clean: bool,

        /// Numero minimo di caratteri per documento
        #[arg(long, default_value_t = 32)]
        min_chars: usize,

        /// Numero massimo caratteri per documento
        #[arg(long, default_value_t = 200_000)]
        max_chars: usize,

        /// Token usato come delimitatore/EOS tra documenti.
        /// Deve esistere nel vocabolario del tokenizer caricato
        /// (es. "<|endoftext|>" per tokenizer stile GPT/GLM, "</s>" per stile Llama/BPE classico).
        #[arg(long, default_value = "<eop>")]
        eos_token: String,

        /// Disabilita il packing tra documenti: ogni documento resta in un record
        /// separato (comportamento precedente). Di default il packing e' attivo:
        /// piu' documenti vengono concatenati (separati da eos_token) fino a
        /// riempire seq_len, per non sprecare capacita' su documenti corti.
        #[arg(long)]
        no_pack: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Train {
            input,
            text_column,
            output,
            vocab_size,
            min_frequency,
            clean,
            min_chars,
        } => train_tokenizer(
            &input,
            &text_column,
            &output,
            vocab_size,
            min_frequency,
            clean,
            min_chars,
        ),
        Command::Encode {
            input,
            tokenizer,
            output,
            text_column,
            seq_len,
            clean,
            min_chars,
            max_chars,
            eos_token,
            no_pack,
        } => encode_parquet(
            &input,
            &tokenizer,
            &output,
            &text_column,
            seq_len,
            clean,
            min_chars,
            max_chars,
            &eos_token,
            !no_pack,
        ),
    }
}

fn expand_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();

    for path in inputs {
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().map(|e| e == "parquet").unwrap_or(false) {
                    out.push(entry.into_path());
                }
            }
        } else {
            out.push(path.clone());
        }
    }

    if out.is_empty() {
        return Err(anyhow!("Nessun file Parquet trovato"));
    }

    out.sort();
    out.dedup();

    Ok(out)
}

fn choose_text_column(files: &[PathBuf], requested: &str) -> Result<String> {
    let file = File::open(&files[0])
        .map_err(|e| anyhow!("Impossibile aprire {:?}: {}", files[0], e))?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| anyhow!("Impossibile leggere {:?}: {}", files[0], e))?;
    let schema = builder.schema().clone();

    if schema.field_with_name(requested).is_ok() {
        return Ok(requested.to_string());
    }

    let preferred = [
        "text",
        "content",
        "body",
        "prompt",
        "input",
        "instruction",
        "output",
        "chunk",
        "passage",
    ];

    for name in preferred {
        if name != requested && schema.field_with_name(name).is_ok() {
            let field = schema.field_with_name(name)?;
            if matches!(field.data_type(), DataType::Utf8 | DataType::LargeUtf8) {
                return Ok(name.to_string());
            }
        }
    }

    for field in schema.fields() {
        if matches!(field.data_type(), DataType::Utf8 | DataType::LargeUtf8) {
            return Ok(field.name().to_string());
        }
    }

    Err(anyhow!(
        "Nessuna colonna testuale trovata. Colonne: {:?}",
        schema.fields().iter().map(|f| f.name().clone()).collect::<Vec<_>>()
    ))
}

fn clean_text(s: &str) -> String {
    s.nfc()
        .collect::<String>()
        .replace('\u{0000}', "")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn clean_for_training(s: &str) -> String {
    let mut text = clean_text(s);

    let url_re = regex::Regex::new(r"https?://\S+|www\.\S+").unwrap();
    let email_re = regex::Regex::new(r"\b[\w.+-]+@[\w-]+\.[\w.-]+\b").unwrap();

    text = url_re.replace_all(&text, " ").to_string();
    text = email_re.replace_all(&text, " ").to_string();

    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn prepare_text(
    value: &str,
    clean: bool,
    min_chars: usize,
    max_chars: usize,
) -> Option<String> {
    let mut text = value.trim().to_string();

    if text.is_empty() {
        return None;
    }

    if clean {
        text = clean_for_training(&text);
    } else {
        text = clean_text(&text);
    }

    let chars = text.chars().count();

    if chars < min_chars {
        return None;
    }

    if chars > max_chars {
        text = text.chars().take(max_chars).collect();
    }

    Some(text)
}

fn read_texts(
    files: &[PathBuf],
    column: &str,
    clean: bool,
    min_chars: usize,
    max_chars: usize,
) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    let total = files.len();
    let mut processed = 0u64;

    for file in files {
        let f = File::open(file)
            .map_err(|e| anyhow!("Impossibile aprire {:?}: {}", file, e))?;

        let builder = ParquetRecordBatchReaderBuilder::try_new(f)
            .map_err(|e| anyhow!("Impossibile leggere {:?}: {}", file, e))?;
        let reader = builder.build().map_err(|e| anyhow!("Impossibile build {:?}: {}", file, e))?;

        for batch in reader {
            let batch = batch.map_err(|e| anyhow!("Errore batch in {:?}: {}", file, e))?;
            let col = batch
                .column_by_name(column)
                .ok_or_else(|| anyhow!("Colonna '{}' non trovata in {:?}", column, file))?;

            if let Some(array) = col.as_string_opt::<i32>() {
                for value in array.iter().flatten() {
                    if let Some(t) = prepare_text(value, clean, min_chars, max_chars) {
                        texts.push(t);
                    }
                }
            } else if let Some(array) = col.as_string_opt::<i64>() {
                for value in array.iter().flatten() {
                    if let Some(t) = prepare_text(value, clean, min_chars, max_chars) {
                        texts.push(t);
                    }
                }
            } else {
                return Err(anyhow!(
                    "La colonna '{}' non è una colonna Utf8/LargeUtf8",
                    column
                ));
            }
        }

        processed += 1;
        eprintln!("  Letto: {processed}/{total} file");
    }

    Ok(texts)
}

fn special_tokens() -> Vec<AddedToken> {
    vec![
        AddedToken::from("<sop>", true),
        AddedToken::from("<eop>", true),
        AddedToken::from("<|assistant|>", true),
        AddedToken::from("<|user|>", true),
        AddedToken::from("<|system|>", true),
        AddedToken::from("<|observation|>", true),
        AddedToken::from("[gMASK]", true),
        AddedToken::from("[sMASK]", true),
        AddedToken::from("[MASK]", true),
        AddedToken::from("<|begin_of_image|>", true),
        AddedToken::from("<|end_of_image|>", true),
        AddedToken::from("<|begin_of_video|>", true),
        AddedToken::from("<|end_of_video|>", true),
        AddedToken::from("<|begin_of_audio|>", true),
        AddedToken::from("<|end_of_audio|>", true),
        AddedToken::from("</think>", true),
        AddedToken::from("<tool_call>", true),
        AddedToken::from("</tool_call>", true),
        AddedToken::from("<tool_response>", true),
        AddedToken::from("</tool_response>", true),
    ]
}

fn train_tokenizer(
    inputs: &[PathBuf],
    text_column: &str,
    output: &Path,
    vocab_size: usize,
    min_frequency: u64,
    clean: bool,
    min_chars: usize,
) -> Result<()> {
    let files = expand_inputs(inputs)?;
    eprintln!("Trovati {} file Parquet", files.len());

    let column = choose_text_column(&files, text_column)?;
    eprintln!("Colonna testo: {}", column);

    eprintln!("Lettura testi...");
    let texts = read_texts(&files, &column, clean, min_chars, 200_000)?;
    eprintln!("Testi validi: {}", texts.len());

    if texts.is_empty() {
        return Err(anyhow!("Nessun testo valido trovato"));
    }

    eprintln!("Addestramento BPE (vocab_size={}, min_freq={})...", vocab_size, min_frequency);

    let mut tokenizer = Tokenizer::new(BPE::default());
    let split = Split::new(
        SplitPattern::Regex(r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+".to_string()),
        SplitDelimiterBehavior::Isolated,
        false,
    ).unwrap();
    // Pre-tokenizer ByteLevel: matchare tokenizer.json (add_prefix_space=false, use_regex=false)
    let byte_level = ByteLevel::default()
        .add_prefix_space(false)
        .use_regex(false);

    // Post-processor ByteLevel: esplicito per matchare tokenizer.json (trim_offsets=false)
    let post_byte_level = tokenizers::pre_tokenizers::byte_level::ByteLevel::default()
        .add_prefix_space(true)
        .trim_offsets(false)
        .use_regex(true);
    tokenizer.with_post_processor(Some(PostProcessorWrapper::ByteLevel(post_byte_level)));
    tokenizer.with_pre_tokenizer(Some(Sequence::new(vec![
        tokenizers::pre_tokenizers::PreTokenizerWrapper::Split(split),
        tokenizers::pre_tokenizers::PreTokenizerWrapper::ByteLevel(byte_level),
    ])));

    for st in special_tokens() {
        tokenizer.add_special_tokens(&[st]);
    }

    let trainer = BpeTrainerBuilder::new()
        .vocab_size(vocab_size)
        .min_frequency(min_frequency)
        .show_progress(true)
        .special_tokens(special_tokens())
        .build();

    let mut trainer_wrapper = TrainerWrapper::BpeTrainer(trainer);

    tokenizer.train(&mut trainer_wrapper, texts.into_iter())
        .map_err(|e| anyhow!("Errore training: {}", e))?;

    eprintln!("Salvataggio tokenizer in {:?}", output);
    tokenizer.save(output, true)
        .map_err(|e| anyhow!("Errore salvataggio: {}", e))?;

    eprintln!("Vocabolario addestrato con successo");

    Ok(())
}

/// Risolve l'id del token EOS/delimitatore nel vocabolario del tokenizer caricato.
/// Fallisce esplicitamente se il token non esiste, invece di ignorarlo in silenzio:
/// un delimitatore di documento sbagliato corromperebbe silenziosamente tutto il
/// dataset binario prodotto.
fn resolve_eos_id(tokenizer: &Tokenizer, eos_token: &str) -> Result<u32> {
    tokenizer.token_to_id(eos_token).ok_or_else(|| {
        anyhow!(
            "Il token EOS '{}' non esiste nel vocabolario di questo tokenizer. \
             Controlla gli 'added_tokens' del tokenizer.json (es. '<|endoftext|>' per \
             tokenizer stile GPT/GLM, '</s>' per tokenizer stile Llama/BPE classico) \
             e passa quello corretto con --eos-token.",
            eos_token
        )
    })
}

/// Encode a single file to a temp file. Returns (doc_count, record_count, token_count).
/// Usa rayon per parallelizzare l'encoding dei batch su tutti i core disponibili.
#[allow(clippy::too_many_arguments)]
fn encode_file(
    file: &Path,
    column: &str,
    tokenizer: Arc<Tokenizer>,
    seq_len: usize,
    clean: bool,
    min_chars: usize,
    max_chars: usize,
    eos_id: u32,
    pack: bool,
    temp_path: &Path,
) -> Result<(u64, u64, u64)> {
    let f = File::open(file)
        .map_err(|e| anyhow!("Impossibile aprire {:?}: {}", file, e))?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(f)
        .map_err(|e| anyhow!("Impossibile leggere {:?}: {}", file, e))?;
    let reader = builder.build().map_err(|e| anyhow!("Impossibile build {:?}: {}", file, e))?;

    // 1) Leggi tutti i batch in memoria (parquet reader non e' thread-safe)
    let batches: Vec<_> = reader
        .collect::<arrow::error::Result<_>>()
        .map_err(|e| anyhow!("Errore lettura batch in {:?}: {}", file, e))?;

    let total_batches = batches.len();
    eprintln!("    {} batch caricati da {:?}", total_batches, file);

    // 2) Estrai testo da ogni batch
    let texts_per_batch: Vec<Vec<String>> = batches
        .iter()
        .map(|batch| {
            let col = batch
                .column_by_name(column)
                .ok_or_else(|| anyhow!("Colonna '{}' non trovata in {:?}", column, file))
                .map_err(|e| anyhow!("{}", e))?;

            let strings: Vec<Option<String>> = if let Some(array) = col.as_string_opt::<i32>() {
                array.iter().map(|v| v.map(|s| s.to_string())).collect()
            } else if let Some(array) = col.as_string_opt::<i64>() {
                array.iter().map(|v| v.map(|s| s.to_string())).collect()
            } else {
                return Err(anyhow!(
                    "La colonna '{}' non è una colonna Utf8/LargeUtf8",
                    column
                ));
            };

            Ok(strings
                .into_iter()
                .filter_map(|opt_text| {
                    let text: String = opt_text?;
                    prepare_text(&text, clean, min_chars, max_chars)
                })
                .collect())
        })
        .collect::<Result<Vec<_>>>()?;

    // 3) Encoding parallelo dei batch con rayon
    let encoded_results: Vec<Vec<Vec<u32>>> = texts_per_batch
        .par_iter()
        .map(|batch_texts| {
            batch_texts
                .iter()
                .map(|text| {
                    let encoding = tokenizer
                        .encode(text.as_str(), false)
                        .map_err(|e| anyhow!("Errore tokenizzazione: {}", e))
                        .unwrap();
                    let mut tokens: Vec<u32> = encoding.get_ids().to_vec();
                    tokens.push(eos_id);
                    tokens
                })
                .collect()
        })
        .collect();

    // 4) Scrivi risultati in sequenza
    let out_file = File::create(temp_path)
        .map_err(|e| anyhow!("Impossibile creare {:?}: {}", temp_path, e))?;
    let mut writer = BufWriter::new(out_file);

    let mut doc_count: u64 = 0;
    let mut record_count: u64 = 0;
    let mut total_tokens: u64 = 0;

    // Buffer di packing
    let mut buffer: Vec<u32> = if pack && seq_len > 0 {
        Vec::with_capacity(seq_len)
    } else {
        Vec::new()
    };

    let write_record = |writer: &mut BufWriter<File>, tokens: &[u32]| -> Result<()> {
        let len = tokens.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        for &t in tokens {
            writer.write_all(&t.to_le_bytes())?;
        }
        Ok(())
    };

    for tokens_vec in &encoded_results {
        for tokens in tokens_vec {
            doc_count += 1;
            total_tokens += tokens.len() as u64;

            if seq_len == 0 {
                write_record(&mut writer, tokens)?;
                record_count += 1;
            } else if pack {
                buffer.extend_from_slice(tokens);
                while buffer.len() >= seq_len {
                    let chunk: Vec<u32> = buffer.drain(0..seq_len).collect();
                    write_record(&mut writer, &chunk)?;
                    record_count += 1;
                }
            } else {
                for chunk in tokens.chunks(seq_len) {
                    write_record(&mut writer, chunk)?;
                    record_count += 1;
                }
            }
        }
    }

    // Flush buffer residuo
    if pack && seq_len > 0 && !buffer.is_empty() {
        write_record(&mut writer, &buffer)?;
        record_count += 1;
    }

    writer.flush()?;
    Ok((doc_count, record_count, total_tokens))
}

#[allow(clippy::too_many_arguments)]
fn encode_parquet(
    inputs: &[PathBuf],
    tokenizer_path: &Path,
    output: &Path,
    text_column: &str,
    seq_len: usize,
    clean: bool,
    min_chars: usize,
    max_chars: usize,
    eos_token: &str,
    pack: bool,
) -> Result<()> {
    let files = expand_inputs(inputs)?;
    eprintln!("Trovati {} file Parquet", files.len());

    let column = choose_text_column(&files, text_column)?;
    eprintln!("Colonna testo: {}", column);

    eprintln!("Caricamento tokenizer da {:?}", tokenizer_path);
    let tokenizer = Arc::new(
        Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("Impossibile caricare {:?}: {}", tokenizer_path, e))?,
    );

    let eos_id = resolve_eos_id(tokenizer.as_ref(), eos_token)?;

    if seq_len > 0 && pack {
        eprintln!("Packing attivo: i documenti verranno concatenati fino a riempire seq_len={}", seq_len);
    } else if seq_len > 0 {
        eprintln!("Packing disattivo: un documento per record, troncato/spezzato a seq_len={}", seq_len);
    } else {
        eprintln!("Modalita' flusso piatto: nessun troncamento, un record per documento");
    }

    // Create temp files for parallel encoding
    let temp_dir = output.parent().unwrap_or(Path::new("."));

    eprintln!("Encoding parallelo su {} file...", files.len());

    // Parallel encoding: each file gets its own temp file
    let results: Vec<_> = files.par_iter().enumerate().map(|(i, file)| {
        let temp_path = temp_dir.join(format!("tokens_temp_{:03}.bin", i));
        eprintln!("  Encoding: {:?} -> {:?}", file, temp_path);
        let result = encode_file(
            file, &column, tokenizer.clone(), seq_len, clean, min_chars, max_chars, eos_id, pack, &temp_path,
        );
        (i, file.clone(), temp_path, result)
    }).collect();

    // Collect results and write final output
    let out_file = File::create(output)
        .map_err(|e| anyhow!("Impossibile creare {:?}: {}", output, e))?;
    let mut writer = BufWriter::new(out_file);

    // Header: magic + num_docs (placeholder)
    let magic: [u8; 4] = *b"PTOK";
    writer.write_all(&magic)?;

    let num_docs_offset = writer.stream_position()?;
    writer.write_all(&0u64.to_le_bytes())?; // placeholder

    let mut total_tokens: u64 = 0;
    let mut total_docs: u64 = 0;
    let mut total_records: u64 = 0;

    // Sort by index to maintain order
    let mut sorted_results = results;
    sorted_results.sort_by_key(|(i, _, _, _)| *i);

    for (_i, file, temp_path, result) in sorted_results {
        match result {
            Ok((doc_count, record_count, token_count)) => {
                eprintln!(
                    "  {:?}: {} doc, {} record, {} token -> {:?}",
                    file, doc_count, record_count, token_count, temp_path
                );
                total_docs += doc_count;
                total_records += record_count;
                total_tokens += token_count;

                // Copy temp file to output
                let mut reader = BufReader::new(File::open(&temp_path)
                    .map_err(|e| anyhow!("Impossibile leggere {:?}: {}", temp_path, e))?);
                std::io::copy(&mut reader, &mut writer)
                    .map_err(|e| anyhow!("Errore copia {:?}: {}", temp_path, e))?;

                // Clean up temp file
                let _ = std::fs::remove_file(&temp_path);
            }
            Err(e) => {
                eprintln!("  ERRORE su {:?}: {}", file, e);
                let _ = std::fs::remove_file(&temp_path);
                return Err(e);
            }
        }
    }

    // Il numero scritto nell'header e' il conteggio dei RECORD nel binario
    // (cio' che il reader dovra' iterare), non il numero di documenti sorgente:
    // con il packing attivo i due numeri divergono, perche' piu' documenti
    // possono confluire nello stesso record.
    writer.seek(SeekFrom::Start(num_docs_offset))?;
    writer.write_all(&total_records.to_le_bytes())?;
    writer.flush()?;

    eprintln!(
        "\nDoc sorgente: {}, Record scritti: {}, Token totali: {}",
        total_docs, total_records, total_tokens
    );
    eprintln!("Output: {:?}", output);

    Ok(())
}
