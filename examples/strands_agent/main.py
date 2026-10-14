import uuid
from datetime import datetime

from strands import Agent, tool
from strands.models.openai import OpenAIModel
from dotenv import load_dotenv

load_dotenv()

model = OpenAIModel(model_id="gpt-4o-mini")


@tool
def get_current_time() -> str:
    """Get the current local date and time."""
    return datetime.now().isoformat(timespec="seconds")


@tool
def add(a: float, b: float) -> float:
    """Add two numbers."""
    return a + b


def new_session():
    session_id = str(uuid.uuid4())
    print(f"[session {session_id}]")
    # an Agent keeps its own message history, so each session gets a fresh one
    return Agent(
        model=model,
        tools=[get_current_time, add],
        system_prompt="You are a helpful assistant. Use your tools when they help.",
        # session.id groups every turn of a session together in the traces
        trace_attributes={"session.id": session_id},
        callback_handler=None,
    )


agent = new_session()
print("Type a message, /new for a new session, /quit to exit.")

while True:
    try:
        user_input = input("> ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        break

    if not user_input:
        continue
    if user_input == "/quit":
        break
    if user_input == "/new":
        agent = new_session()
        continue

    print(agent(user_input))
