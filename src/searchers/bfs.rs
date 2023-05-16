use crate::cli_pretty_printing::decoded_how_many_times;
use crate::filtration_system::MyResults;
use crossbeam::channel::Sender;

use log::trace;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::DecoderResult;

/// Breadth first search is our search algorithm
/// https://en.wikipedia.org/wiki/Breadth-first_search
pub fn bfs(input: String, result_sender: Sender<Option<DecoderResult>>, stop: Arc<AtomicBool>) {
    let initial = DecoderResult {
        text: input,
        path: vec![],
    };
    let mut seen_strings = HashSet::new();
    // all strings to search through
    let mut current_strings = vec![initial];

    let mut curr_depth: u32 = 1; // as we have input string, so we start from 1

    // loop through all of the strings in the vec
    while !current_strings.is_empty() && !stop.load(std::sync::atomic::Ordering::Relaxed) {
        trace!("Number of potential decodings: {}", current_strings.len());
        trace!("Current depth is {:?}", curr_depth);

        let mut new_strings: Vec<DecoderResult> = vec![];

        current_strings.into_iter().try_for_each(|current_string| {
            let res = super::perform_decoding(&current_string);

            match res {
                // if it's Break variant, we have cracked the text successfully
                // so just stop processing further.
                MyResults::Break(mut res) => {
                    let mut decoders_used = current_string.path;
                    // succesfully result must have only one string as unencrypted_text
                    let text = res.unencrypted_text.as_mut().and_then(|s| s.pop()).unwrap();
                    decoders_used.push(res);
                    let result_text = DecoderResult {
                        text,
                        path: decoders_used,
                    };

                    decoded_how_many_times(curr_depth);
                    result_sender
                        .send(Some(result_text))
                        .expect("Should succesfully send the result");

                    // stop further iterations
                    stop.store(true, std::sync::atomic::Ordering::Relaxed);
                    None // short-circuits the iterator
                }
                MyResults::Continue(results_vec) => {
                    new_strings.extend(
                        results_vec
                            .into_iter()
                            .flat_map(|mut r| {
                                let mut decoders_used = current_string.path.clone();
                                // text is a vector of strings
                                let mut text = r.unencrypted_text.take().unwrap_or_default();

                                text.retain(|s| {
                                    !check_if_string_cant_be_decoded(s)
                                        && seen_strings.insert(s.clone())
                                });

                                if text.is_empty() {
                                    return None;
                                }

                                decoders_used.push(r);
                                Some(text.into_iter().map(move |t| DecoderResult {
                                    text: t,
                                    path: decoders_used.to_vec(),
                                }))
                            })
                            .flatten(),
                    );
                    Some(()) // indicate we want to continue processing
                }
            }
        });

        current_strings = new_strings;
        curr_depth += 1;

        trace!("Refreshed the vector, {:?}", current_strings);
    }
    result_sender.try_send(None).ok();
}

/// If this returns False it will not attempt to decode that string
fn check_if_string_cant_be_decoded(text: &str) -> bool {
    text.len() <= 2
}

#[cfg(test)]
mod tests {
    use crossbeam::channel::bounded;

    use super::*;

    #[test]
    fn bfs_succeeds() {
        // this will work after english checker can identify "CANARY: hello"
        let (tx, rx) = bounded::<Option<DecoderResult>>(1);
        let stopper = Arc::new(AtomicBool::new(false));
        bfs("b2xsZWg=".into(), tx, stopper);
        let result = rx.recv().unwrap();
        assert!(result.is_some());
        let txt = result.unwrap().text;
        assert!(txt == "hello");
    }

    // Vector storing the strings to perform decoding in next iteraion
    // had strings only from result of last decoding it performed.
    // This was due to reassignment in try_for_each block
    // which lead to unintended behaviour.
    // We want strings from all results, so to fix it,
    // we call .extend() to extend the vector.
    // Link to forum https://discord.com/channels/754001738184392704/1002135076034859068
    // This also tests the bug whereby each iteration of caesar was not passed to the next decoder
    // So in Ciphey only Rot1(X) was passed to base64, not Rot13(X)
    #[test]
    fn non_deterministic_like_behaviour_regression_test() {
        // Caesar Cipher (Rot13) -> Base64
        let (tx, rx) = bounded::<Option<DecoderResult>>(1);
        let stopper = Arc::new(AtomicBool::new(false));
        bfs("MTkyLjE2OC4wLjE=".into(), tx, stopper);
        let result = rx.recv().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "192.168.0.1");
    }

    #[test]
    fn string_size_checker_returns_bad_if_string_cant_be_decoded() {
        // Should return true because it cant decode it
        let text = "12";
        assert!(check_if_string_cant_be_decoded(text));
    }

    #[test]
    fn string_size_checker_returns_ok_if_string_can_be_decoded() {
        // Should return true because it cant decode it
        let text = "123";
        assert!(!check_if_string_cant_be_decoded(text));
    }
}
