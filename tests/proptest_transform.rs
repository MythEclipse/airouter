use proptest::prelude::*;
use airouter::types::openai::{ChatCompletionRequest, Message, Content};
use airouter::transform::openai_to_anthropic::convert_chat_request;

fn arb_text() -> impl Strategy<Value = Content> {
    "[a-zA-Z0-9 ,.!?]{0,20}".prop_map(Content::Text)
}

fn arb_role() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("user".to_string()),
        Just("assistant".to_string()),
        Just("system".to_string()),
    ]
}

fn arb_message() -> impl Strategy<Value = Message> {
    (arb_role(), arb_text()).prop_map(|(role, content)| Message {
        role,
        content: Some(content),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    })
}

fn arb_request() -> impl Strategy<Value = ChatCompletionRequest> {
    (
        "[a-zA-Z0-9_-]{1,10}",
        prop::collection::vec(arb_message(), 1..4),
    )
        .prop_map(|(model, messages)| ChatCompletionRequest {
            model,
            messages,
            stream: Some(false),
            ..Default::default()
        })
}

proptest! {
    #[test]
    fn openai_to_anthropic_preserves_message_count(req in arb_request()) {
        let anthropic_req = convert_chat_request(&req).unwrap();
        let non_system = req.messages.iter().filter(|m| m.role != "system").count();
        assert_eq!(
            anthropic_req.messages.len(),
            non_system,
            "message count (minus system) should be preserved"
        );
    }

    #[test]
    fn openai_to_anthropic_accepts_all_requests(req in arb_request()) {
        let result = convert_chat_request(&req);
        assert!(result.is_ok(), "all valid OpenAI requests should convert");
    }
}
